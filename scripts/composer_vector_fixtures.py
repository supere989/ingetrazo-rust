#!/usr/bin/env python3
"""Build named native paper consumers together; reuse only explicit caller builds."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

EXAMPLES = ("sheet_view_pdf_fixture", "paper_boundary_fixture", "qt_paper_numeral_fixture")
TEXT_EXAMPLES = ("paper_font_resolver_fixture", "paper_paragraph_fixture", "composer_text_pdf_fixture", "composer_pdf_ui_fixture")
WORKER = "ingetrazo-render-worker"


def consumers(directory, *, include_section=False, include_text=False):
    if os.environ.get("CARGO_BUILD_TARGET") or os.environ.get("CARGO_PROFILE"):
        raise ValueError("Paper acceptance requires native debug executables; unset CARGO_BUILD_TARGET/CARGO_PROFILE")
    directory = Path(directory).resolve()
    suffix = ".exe" if os.name == "nt" else ""
    names = EXAMPLES + (("paper_section_fixture",) if include_section else ()) + (TEXT_EXAMPLES if include_text else ())
    paths = {name: directory / "examples" / (name + suffix) for name in names}
    paths[WORKER] = directory / (WORKER + suffix)
    return paths


def require_consumers(paths):
    for path in paths.values():
        if not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError(f"Required prebuilt executable missing or not executable: {path}")
    return paths


def build(*, include_section=False, include_text=False):
    target = Path(os.environ.get("CARGO_TARGET_DIR", "target")).resolve()
    paths = consumers(target / "debug", include_section=include_section, include_text=include_text)
    command = ["cargo", "build", "--locked", "--message-format=json-render-diagnostics", "-j2", "--target-dir", str(target)]
    for package in ("ingetrazo-gui", "ingetrazo-render", "ingetrazo-formats"):
        command += ["-p", package]
    for name in paths:
        command += ["--bin" if name == WORKER else "--example", name]
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE, text=True)
    artifacts = {name: set() for name in paths}
    for line in result.stdout.splitlines():
        message = json.loads(line)
        name = message.get("target", {}).get("name")
        kind = "bin" if name == WORKER else "example"
        if (message.get("reason") == "compiler-artifact" and name in artifacts
                and message.get("executable") and kind in message["target"]["kind"]):
            artifacts[name].add(Path(message["executable"]).resolve())
    for name, expected in paths.items():
        if artifacts[name] != {expected.resolve()}:
            raise ValueError(f"Unsupported target configuration or missing Cargo artifact for {name}: {sorted(map(str, artifacts[name]))}; expected {expected}")
    return require_consumers(paths)


def provenance(paths):
    return {name: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for name, path in paths.items()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--include-text", action="store_true", help="Build the font, paragraph and composer UI consumers in the same Cargo invocation")
    args = parser.parse_args()
    print(json.dumps({"consumers": provenance(build(include_section=True, include_text=args.include_text)), "cargo_builds": 1}, indent=2))
