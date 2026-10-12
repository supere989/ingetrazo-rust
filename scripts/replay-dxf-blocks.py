#!/usr/bin/env python3
"""Replay frozen BLOCKS/INSERT workflows through the prebuilt app consumer."""
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
    parser.add_argument("--python", default=sys.executable)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    folder = repo / "docs/audits/dxf-blocks"
    binary, out = args.binary.resolve(), args.output.resolve()
    assert not out.exists(), "Retain each replay in a fresh directory"
    binary_hash = digest(binary)
    out.mkdir(parents=True)
    pin = subprocess.check_output(["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True).strip()
    result = {"revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
              "binary_sha256": binary_hash, "source_pin": pin, "archives": {},
              "fresh_source_captures": 0, "commands": [], "result": "incomplete"}
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out / "state"),
               XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"))

    def archive(manifest_name, archive_name, selected=None):
        manifest = json.loads((folder / manifest_name).read_text())
        assert pin == manifest.get("source_pin", manifest.get("source_commit"))
        path = folder / archive_name
        assert digest(path) == manifest["archive_sha256"]
        records = manifest.get("entries", manifest.get("members"))
        if isinstance(records, list):
            rows = records
            records = {row["path"]: row for row in rows}
            assert len(records) == len(rows)
        with zipfile.ZipFile(path) as bundle:
            assert len(bundle.namelist()) == len(records) and set(bundle.namelist()) == set(records)
            if selected is not None:
                assert set(selected) <= set(records)
            for name, expected in records.items():
                data = bundle.read(name)
                assert len(data) == expected["bytes"] and hashlib.sha256(data).hexdigest() == expected["sha256"], name
                if selected is None or name in selected:
                    target = out / Path(name).name
                    assert not target.exists(), "Duplicate flattened archive member: " + name
                    target.write_bytes(data)
        result["archives"][archive_name] = manifest["archive_sha256"]
        return manifest

    def run(argv, log):
        command = [str(value) for value in argv]
        row = {"argv": command, "log": log}
        result["commands"].append(row)
        with (out / log).open("w") as stdout, (out / (log + ".stderr")).open("w") as stderr:
            proc = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=120)
        row["returncode"] = proc.returncode
        proc.check_returncode()

    try:
        archive("source-manifest.json", "source-records.zip")
        archive("addendum-source-manifest.json", "addendum-source-records.zip")
        evidence = archive("manifest.json", "records.zip", ["native/first-native-dxf-blocks.json", "native/first-native-dxf-blocks-addendum.json"])
        # The historical Git bindings were independently authenticated at
        # integration. CI may have a shallow checkout; bind its current sources
        # while preserving the accepted historical identity in the manifest.
        result["retained_runtime_commit"] = evidence["runtime_commit"]
        result["current_sources"] = {name: digest(repo / name) for name in evidence["sources"]}
        for suffix in ["", "-addendum"]:
            run([binary, out, "--dxf-blocks" + suffix], "runtime" + suffix + ".log")
            run([args.python, repo / "scripts/reference-dxf-blocks-reopen.py", out] + (["--addendum"] if suffix else []), "reopen" + suffix + ".log")
        run([args.python, repo / "scripts/check-dxf-blocks.py", out], "acceptance.json")
        result["acceptance"] = json.loads((out / "acceptance.json").read_text())
        assert result["acceptance"]["passed"] is True
        result["result"] = "pass"
    except Exception as error:
        result["result"] = "fail"
        result["error"] = str(error)
        raise
    finally:
        (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
