#!/usr/bin/env python3
"""Replay frozen figure and ordinary GLB exports using one prebuilt consumer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    folder = repo / "docs/audits/gltf-figures"
    manifest = json.loads((folder / "manifest.json").read_text())
    binary = args.binary.resolve()
    out = args.output.resolve()
    assert not out.exists(), "Retain each replay in a new directory"
    assert subprocess.check_output(
        ["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True
    ).strip() == manifest["source_pin"]
    for name, expected in manifest["original_sources"].items():
        assert hashlib.sha256((repo / "ingetrazo" / name).read_bytes()).hexdigest() == expected, name
    for name, expected in manifest["old9_source_files"].items():
        assert hashlib.sha256((repo / name).read_bytes()).hexdigest() == expected, name
    archive = folder / "records.zip"
    with archive.open("rb") as stream:
        assert hashlib.file_digest(stream, "sha256").hexdigest() == manifest["archive_sha256"]
    out.mkdir(parents=True)
    retained_failures = {
        "native/initial-decoded-failure.json",
        "native/path-only-candidate-failure.json",
    }
    with zipfile.ZipFile(archive) as records:
        assert set(records.namelist()) == set(manifest["members"])
        for name, expected in manifest["members"].items():
            data = records.read(name)
            assert len(data) == expected["bytes"]
            assert hashlib.sha256(data).hexdigest() == expected["sha256"], name
            if (name.startswith("source/") and not name.startswith("source/worker/")) or name in retained_failures:
                target = out / name
                target.resolve().relative_to(out)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
    source, native = out / "source", out / "native"
    cases = json.loads((source / "cases.json").read_text())
    commands = []
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen")

    def run(argv, log):
        argv = [str(v) for v in argv]
        commands.append(argv)
        with (out / log).open("w") as stdout, (out / (log + ".stderr")).open("w") as stderr:
            subprocess.run(argv, cwd=repo, env=env, stdout=stdout, stderr=stderr, check=True, timeout=120)

    for case in cases:
        name = case["name"]
        assert Path(name).name == name and name not in {".", ".."}
        run([binary, source / (name + ".igz"), native / (name + ".glb"), source / (name + ".camera.json")], name + "-camera.log")
        run([binary, source / (name + ".igz"), native / (name + ".ordinary.glb")], name + "-ordinary.log")
    run([binary, source / "collapse.igz", native / "collapse-unused.glb", source / "collapse.camera.json", "--refusal"], "native/collapse-refusal.json")
    run([sys.executable, repo / "scripts/check-gltf-figures.py", source, native], "figures-check.json")
    old_source = repo / "docs/audits/gltf-rich/original"
    old_native = out / "ordinary-rich"
    old_native.mkdir()
    for name in (old_source / "cases.txt").read_text().splitlines():
        assert Path(name).name == name and name not in {".", ".."}
        run([binary, old_source / (name + ".igz"), old_native / (name + ".glb")], name + "-rich.log")
    run([sys.executable, repo / "scripts/check-gltf-rich.py", old_source, old_native], "rich-check.log")
    figures = json.loads((out / "figures-check.json").read_text())
    result = {
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "source_pin": manifest["source_pin"],
        "source_archive_sha256": manifest["archive_sha256"],
        "figures": figures,
        "ordinary_rich_cases": 9,
        "ordinary_rich_negative_controls": 7,
        "fresh_source_captures": 0,
        "commands": commands,
        "result": "pass",
        "scope": "Frozen decoded GLB and writer refusal acceptance. Actual Blender worker proof is retained separately; no new Blender render in this replay.",
    }
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k != "commands"}))


if __name__ == "__main__":
    main()
