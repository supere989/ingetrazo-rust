#!/usr/bin/env python3
"""Replay the first actual macOS text witness; never capture the original again."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import zipfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("worker", type=Path)
    parser.add_argument("font_probe", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    assert sys.platform == "darwin", "Native platform replay requires macOS"
    repo = Path(__file__).resolve().parents[1]
    machine = platform.machine()
    assert machine in {"arm64", "x86_64"}
    archive = repo / "docs/audits/sheet-metal-allocation" / ("first-arm-text" if machine == "arm64" else "first-intel-text")
    manifest = json.loads((archive / "manifest.json").read_text())
    zipped = archive / "records.zip"
    assert digest(zipped.read_bytes()) == manifest["archive_sha256"]
    out = args.output.resolve()
    assert not out.exists(), "Use a fresh native result directory; preserve first failures"
    out.mkdir(parents=True)
    with zipfile.ZipFile(zipped) as z:
        assert len(z.namelist()) == len(set(z.namelist())) == len(manifest["entries"])
        assert set(z.namelist()) == {v["path"] for v in manifest["entries"]}
        for v in manifest["entries"]:
            data = z.read(v["path"])
            assert len(data) == v["bytes"] and digest(data) == v["sha256"]
            if v["path"].startswith("source/"):
                relative = Path(v["path"])
                assert not relative.is_absolute() and ".." not in relative.parts
                target = out / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
    source = out / "source"
    receipt = json.loads((source / "receipt.json").read_text())
    assert receipt["machine"] == machine and receipt["qt"] == "6.12.0"
    pin = "6be29fe437a7cd992d4e079f71720821b25216b9"
    assert receipt["source_pin"] == pin
    assert subprocess.check_output(["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True).strip() == pin
    for name, expected in receipt["source_files"].items():
        assert digest((repo / "ingetrazo" / name).read_bytes()) == expected
    binaries = {name: path.resolve() for name, path in [("consumer", args.binary), ("worker", args.worker), ("font_probe", args.font_probe)]}
    identities = {name: digest(path.read_bytes()) for name, path in binaries.items()}
    env = os.environ.copy()
    env.update(QT_QPA_PLATFORM="offscreen", XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"), INGETRAZO_STATE_DIR=str(out / "state"))
    result = {"archive_sha256": manifest["archive_sha256"], "binaries": identities, "fresh_source_images": 0,
              "source_receipt_sha256": digest((source / "receipt.json").read_bytes()), "machine": machine,
              "scope": "Strict whole-image replay of first actual platform witness; source and native font metadata compared independently."}
    with (out / "font.stderr").open("w") as stderr:
        probe = subprocess.run([str(binaries["font_probe"])], cwd=repo, env=env, stdout=subprocess.PIPE, stderr=stderr, timeout=60)
    result["font_returncode"] = probe.returncode
    if probe.returncode == 0:
        font = json.loads(probe.stdout)
        tables = {name: {"bytes": len(bytes.fromhex(data)), "sha256": digest(bytes.fromhex(data))} for name, data in font.pop("table_hex").items()}
        assert set(tables) == {"head", "hhea", "maxp", "OS/2", "name", "cmap", "hmtx"}
        font["tables"] = tables
        font["tables_exact"] = all(receipt["font"]["tables"][name] == value for name, value in tables.items())
        font["metrics_exact"] = all(font[name] == receipt["font"][name] for name in ("ascent", "descent"))
        (out / "font.json").write_text(json.dumps(font, indent=2) + "\n")
        result["font_identity_exact"] = font["tables_exact"] and font["metrics_exact"]
    else:
        result["font_identity_exact"] = False
        (out / "font.stdout").write_bytes(probe.stdout)
    native = out / "native"
    with (out / "runtime.log").open("w") as stdout, (out / "runtime.stderr").open("w") as stderr:
        run = subprocess.run([str(binaries["consumer"]), str(source), str(native), "--worker", str(binaries["worker"])], cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=120)
    result["runtime_returncode"] = run.returncode
    if run.returncode == 0:
        spec = importlib.util.spec_from_file_location("sheet_image_check", repo / "scripts/check-sheet-image-native.py")
        check = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(check)
        original_image = check.Image.open(source / "text.png")
        native_image = check.Image.open(native / "text.png")
        a, b = original_image.tobytes(), native_image.tobytes()
        assert original_image.size == native_image.size and len(a) == len(b)
        assert digest(a) == receipt["rgb_sha256"]
        record = json.loads((native / "native.json").read_text())
        assert [v["case"] for v in record["images"]] == ["text"]
        assert record["images"][0]["file"] == "text.png"
        assert tuple(record["images"][0]["pixels"]) == original_image.size == (827, 1169)
        assert record["images"][0]["dpi_remembered"] == record["images"][0]["metrics"]["dpi"] == 100
        result.update(different_pixels=sum(a[i:i+3] != b[i:i+3] for i in range(0, len(a), 3)),
                      max_channel_error=max((abs(x-y) for x, y in zip(a,b)), default=0),
                      physical_headers_exact=check.physical(source / "text.png") == check.physical(native / "text.png"), native_rgb_sha256=digest(b))
    result["passed"] = result["font_identity_exact"] and run.returncode == 0 and result.get("different_pixels") == 0 and result.get("physical_headers_exact")
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    raise SystemExit(0 if result["passed"] else 1)


if __name__ == "__main__":
    main()
