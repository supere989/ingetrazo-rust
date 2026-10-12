#!/usr/bin/env python3
"""Replay authenticated physical source images through a prebuilt native GPU consumer."""
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
    parser.add_argument("worker", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--software", action="store_true")
    parser.add_argument("--skip-text", action="store_true", help="Diagnostic geometry/codec scope; platform font parity remains open")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    folder = repo / "docs/audits/sheet-image-export"
    binary, worker, out = args.binary.resolve(), args.worker.resolve(), args.output.resolve()
    assert not out.exists(), "Retain each replay in a fresh directory"
    identities = {"consumer": digest(binary), "worker": digest(worker)}
    out.mkdir(parents=True)
    pin = subprocess.check_output(["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True).strip()
    archives = {}

    def source(manifest_name, archive_name, destination):
        manifest = json.loads((folder / manifest_name).read_text())
        assert pin == manifest["source_pin"]
        archive = folder / archive_name
        assert digest(archive) == manifest["archive_sha256"]
        entries = manifest.get("entries", manifest.get("members"))
        if isinstance(entries, list):
            records = {entry["path"]: entry for entry in entries}
            assert len(records) == len(entries)
        else:
            records = entries
        assert isinstance(records, dict)
        with zipfile.ZipFile(archive) as bundle:
            assert set(bundle.namelist()) == set(records)
            for name, expected in records.items():
                data = bundle.read(name)
                assert len(data) == expected["bytes"] and hashlib.sha256(data).hexdigest() == expected["sha256"], name
                path = destination / name
                path.resolve().relative_to(destination)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        archives[archive_name] = manifest["archive_sha256"]

    source("source-manifest.json", "source-records.zip", out / "source")
    source("view-source-manifest.json", "view-source-records.zip", out / "views")
    native = out / "native"
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out / "state"),
               XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"))
    commands = []

    def run(argv, log):
        command = [str(v) for v in argv]
        commands.append(command)
        with (out / log).open("w") as stdout, (out / (log + ".stderr")).open("w") as stderr:
            subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, check=True, timeout=120)

    flags = ["--skip-text"] if args.skip_text else []
    run([binary, out / "source", native, "--views", out / "views", "--worker", worker] + flags + (["--software"] if args.software else []), "runtime.log")
    run([sys.executable, repo / "scripts/check-sheet-image-native.py", out / "source", native, out / "comparison.json", out / "views"] + flags, "checker.log")
    report = json.loads((out / "comparison.json").read_text())
    result = {"revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
              "binaries": identities, "source_pin": pin, "source_archives": archives,
              "comparison": report, "fresh_source_captures": 0, "commands": commands,
              "software_requested": args.software, "platform": sys.platform,
              "result": "pass", "scope": "Actual native export of the frozen physical image cases. Geometry-only Mac diagnostics exclude text; no full painter, desktop chooser or representative performance claim."}
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k != "commands"}))


if __name__ == "__main__":
    main()
