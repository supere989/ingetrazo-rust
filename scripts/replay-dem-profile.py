#!/usr/bin/env python3
"""Replay frozen original DEM observations through one prebuilt App consumer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import zipfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def execute(command, cwd, env, stdout, stderr):
    # The finite HTTP driver owns a native child. On a POSIX timeout retire the
    # entire group we created, so a failed acceptance run leaves no tile worker.
    with subprocess.Popen(command, cwd=cwd, env=env, stdout=stdout, stderr=stderr,
                          start_new_session=os.name == "posix") as process:
        try:
            return process.wait(timeout=180)
        except subprocess.TimeoutExpired:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
            process.wait()
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    folder = repo / "docs/audits/dem-profile"
    binary, out = args.binary.resolve(), args.output.resolve()
    assert not out.exists(), "Each replay requires a fresh output directory"
    out.mkdir(parents=True)
    work = out / "workflow"
    work.mkdir()
    result = {"result": "incomplete", "fresh_source_captures": 0, "commands": []}
    try:
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
                path = work / name
                path.resolve().relative_to(work)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        result.update(revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
                      binary_sha256=digest(binary), source_pin=pin,
                      archive_sha256=manifest["archive_sha256"], source_members=len(records))
        env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out / "state"),
                   XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"))
        comparison = out / "comparison.json"
        commands = [
            ([sys.executable, repo / "scripts/run-dem-profile.py", binary, work], "runtime.log"),
            ([sys.executable, repo / "scripts/reference-dem-profile-reopens.py", work, work], "original-reopens.log"),
            ([sys.executable, repo / "scripts/check-dem-profile.py", work, work, comparison], "checker.log"),
        ]
        for argv, log in commands:
            command = [str(value) for value in argv]
            started = time.monotonic()
            record = {"argv": command, "cwd": str(repo), "returncode": None}
            result["commands"].append(record)
            try:
                with (out / log).open("w") as stdout, (out / (log + ".stderr")).open("w") as stderr:
                    code = execute(command, repo, env, stdout, stderr)
                record["returncode"] = code
                if comparison.exists():
                    result["comparison"] = json.loads(comparison.read_text())
                if code != 0:
                    raise subprocess.CalledProcessError(code, command)
            finally:
                record["seconds"] = time.monotonic() - started
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
