#!/usr/bin/env python3
"""Retain test results and shallow original-to-Rust IGZ roundtrip evidence.
Not a semantic parity checker: missing fields and entity counts are signals only.
Run inside devbox from repository root after initializing submodules.
"""
import json, subprocess, tempfile, zipfile
from pathlib import Path
REQUIRED = {"arco-yanque.igz", "banca-pergola-yanque.igz", "luminaria-solar-yanque.igz", "pileta-fuente-yanque.igz"}
missing = REQUIRED - {p.name for p in Path("ingetrazo/examples").glob("*.igz")}
if missing:
    raise SystemExit("Required original fixtures missing: " + ", ".join(sorted(missing)))
OUT = Path("docs/audits")
OUT.mkdir(parents=True, exist_ok=True)
def read_doc(path):
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as z:
            data = json.loads(z.read("document.json"))
    else:
        data = json.loads(path.read_text())
    return data.get("scene", data)
def run(args):
    p = subprocess.run(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return p.returncode, p.stdout
code, log = run(["cargo", "test", "--workspace", "--locked"])
(OUT / "test-results.txt").write_text(log.rstrip() + "\n")
report = {"test_exit_code": code, "test_summaries": [s for s in log.splitlines() if s.startswith("test result:")], "fixtures": [], "limitations": "Shallow field/count comparison only; no semantic, visual, or Python-reopen validation."}
build, log = run(["cargo", "build", "--locked", "-p", "ingetrazo-cli"])
(OUT / "build-results.txt").write_text(log.rstrip() + "\n")
report["build_exit_code"] = build
if build == 0:
    code, log = run(["target/debug/ingetrazo", "--check"])
    report["diagnostics"] = {"exit_code": code, "output": log}
    with tempfile.TemporaryDirectory() as tmp:
        for path in sorted(Path("ingetrazo/examples").glob("*.igz")):
            dest = Path(tmp) / path.name
            code, log = run(["target/debug/ingetrazo", "convert", str(path), str(dest)])
            row = {"name": path.name, "exit_code": code, "output": log}
            if code == 0:
                a, b = read_doc(path), read_doc(dest)
                row["missing_nonempty_fields"] = sorted(k for k, v in a.items() if v and k not in b)
                row["list_counts"] = {k: {"original": len(v), "roundtrip": len(b.get(k, [])) if isinstance(b.get(k, []), list) else None} for k, v in a.items() if isinstance(v, list)}
            report["fixtures"].append(row)
(OUT / "parity-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
raise SystemExit(1 if report["test_exit_code"] or build or any(r["exit_code"] for r in report["fixtures"]) else 0)
