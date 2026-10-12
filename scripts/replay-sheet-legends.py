#!/usr/bin/env python3
"""Strict frozen sheet legend replay through a prebuilt physical exporter."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--software", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    folder = repo / "docs/audits/sheet-legends"
    binary, out = args.binary.resolve(), args.output.resolve()
    assert not out.exists(), "Each replay requires a fresh output directory"
    out.mkdir(parents=True)
    source, native = out / "source", out / "native"
    source.mkdir()
    manifest = json.loads((folder / "source-manifest.json").read_text())
    pin = subprocess.check_output(["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True).strip()
    assert pin == manifest["original_commit"]
    archive = folder / "source-records.zip"
    assert digest(archive) == manifest["archive_sha256"]
    records = manifest["members"]
    with zipfile.ZipFile(archive) as bundle:
        assert len(bundle.namelist()) == len(records) and set(bundle.namelist()) == set(records)
        for name, expected in records.items():
            data = bundle.read(name)
            assert len(data) == expected["size"] and hashlib.sha256(data).hexdigest() == expected["sha256"], name
            path = source / name
            path.resolve().relative_to(source)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out / "state"),
               XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"))
    result = {"revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
              "binary_sha256": digest(binary), "source_pin": pin, "archive_sha256": manifest["archive_sha256"],
              "fresh_source_captures": 0, "commands": [], "result": "incomplete"}
    try:
        for argv, log in [([binary, source, native] + (["--software"] if args.software else []), "runtime.log"),
                          ([sys.executable, repo / "scripts/check-sheet-legends.py", source, native, out / "comparison.json"], "checker.log")]:
            command = [str(v) for v in argv]
            with (out / log).open("w") as stdout, (out / (log + ".stderr")).open("w") as stderr:
                run = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=180)
            result["commands"].append({"argv": command, "returncode": run.returncode})
            if (out / "comparison.json").exists():
                result["comparison"] = json.loads((out / "comparison.json").read_text())
            run.check_returncode()
        result["result"] = "pass"
    except Exception as error:
        result["result"] = "fail"
        result["error"] = str(error)
    finally:
        (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return 0 if result["result"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
