#!/usr/bin/env python3
"""Replay frozen offline PhotoMesh profiles through one prebuilt App consumer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import zipfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    folder = repo / "docs/audits/survey-profile"
    binary, out = args.binary.resolve(), args.output.resolve()
    assert not out.exists(), "Each replay requires a fresh output directory"
    out.mkdir(parents=True)
    work = out / "workflow"
    work.mkdir()
    result = {"result": "incomplete", "fresh_source_captures": 0, "commands": []}
    try:
        import numpy
        manifest = json.loads((folder / "source-manifest.json").read_text())
        pin = subprocess.check_output(["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True).strip()
        assert pin == manifest["source_commit"]
        assert numpy.__version__ == manifest["numpy"]
        archive = folder / "source-records.zip"
        assert digest(archive) == manifest["archive_sha256"]
        records = manifest["members"]
        with zipfile.ZipFile(archive) as bundle:
            assert len(bundle.namelist()) == len(records) and set(bundle.namelist()) == set(records)
            for name, expected in records.items():
                data = bundle.read(name)
                assert len(data) == expected["bytes"] and hashlib.sha256(data).hexdigest() == expected["sha256"], name
                path = work / name
                path.resolve().relative_to(work)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        result.update(revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
                      binary_sha256=digest(binary), source_pin=pin, numpy=numpy.__version__,
                      archive_sha256=manifest["archive_sha256"], source_members=len(records))
        env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out / "state"),
                   XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"))
        for argv, log in [([binary, work, "--survey-profile"], "runtime.log"),
                          ([sys.executable, repo / "scripts/reference-survey-profile-reopens.py", work], "original-reopens.log"),
                          ([sys.executable, repo / "scripts/check-survey-profile.py", work], "comparison.json")]:
            command = [str(v) for v in argv]
            started = time.monotonic()
            with (out / log).open("w") as stdout, (out / (log + ".stderr")).open("w") as stderr:
                run = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=180)
            result["commands"].append({"argv": command, "returncode": run.returncode, "seconds": time.monotonic() - started})
            run.check_returncode()
        result["comparison"] = json.loads((out / "comparison.json").read_text())
        assert result["comparison"]["passed"] is True
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
