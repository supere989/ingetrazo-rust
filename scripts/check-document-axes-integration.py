#!/usr/bin/env python3
"""Replay integrated app consumers against authenticated, unchanged source inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binaries", type=Path, help="prebuilt example directory")
parser.add_argument("output", type=Path)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[1]
binaries = args.binaries.resolve()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
pin = "6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git", "-C", str(repo/"ingetrazo"), "rev-parse", "HEAD"], text=True).strip() == pin
suffix = ".exe" if os.name == "nt" else ""
consumers = ["change_axes_fixture", "document_preservation_workflow"]
identities = {name: hashlib.sha256((binaries/(name+suffix)).read_bytes()).hexdigest() for name in consumers}
env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out/"state"),
           XDG_CONFIG_HOME=str(out/"config"), XDG_DATA_HOME=str(out/"data"), XDG_CACHE_HOME=str(out/"cache"))

def archive(name, manifest_name, selected, destination):
    folder = repo/"docs/audits"/name
    manifest = json.loads((folder/manifest_name).read_text())
    raw = (folder/"records.zip").read_bytes()
    assert hashlib.sha256(raw).hexdigest() == manifest["archive_sha256"], name
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(folder/"records.zip") as z:
        assert set(z.namelist()) == set(manifest["records"]), name
        for key, expected in manifest["records"].items():
            blob = z.read(key)
            digest = expected if isinstance(expected, str) else expected["sha256"]
            assert hashlib.sha256(blob).hexdigest() == digest, key
            if isinstance(expected, dict):
                assert len(blob) == expected["bytes"], key
            if key in selected:
                (destination/Path(key).name).write_bytes(blob)
        assert set(selected) <= set(z.namelist()), name
    return manifest["archive_sha256"]

def run(argv, output):
    with (out/output).open("w") as stdout, (out/(output+".stderr")).open("w") as stderr:
        subprocess.run([str(v) for v in argv], cwd=repo, env=env, stdout=stdout, stderr=stderr, check=True, timeout=120)

axes = out/"axes"
axis_cases = ["classic", "shared", "nested", "oblique", "context"]
a = archive("change-axes", "provenance.json", [c+".input.igz" for c in axis_cases]+["original.json"], axes)
doc = out/"documents"
d = archive("document-preservation-workflow", "manifest.json", ["capture/retained.igz", "capture/nested-leaves.igz", "capture/expected.json"], doc)
run([binaries/(consumers[0]+suffix), axes], "axes/native.json")
run([sys.executable, repo/"scripts/reference-change-axes.py", axes, "inspect"], "axes/reopened.json")
run([sys.executable, repo/"scripts/check-change-axes.py", axes], "axes/check.json")
run([binaries/(consumers[1]+suffix), doc], "documents/native.json")
run([sys.executable, repo/"scripts/reference-document-preservation-workflow.py", "compare", doc], "documents/compare.json")
run([sys.executable, repo/"scripts/check-document-preservation-workflow.py", doc/"observed.json"], "documents/check.log")
rows = json.loads((doc/"native.json").read_text())["rows"]
assert [(x["fixture"],x["phase"]) for x in rows] == [(f,p) for f in ["retained","nested-leaves"] for p in ["edited","undo","redo"]]
assert all(x["saved"] and x["clean"] for x in rows)
result = {"revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip(),
          "source_pin":pin,"binary_sha256":identities,"source_archives":{"axes":a,"documents":d},
          "axes":json.loads((axes/"check.json").read_text()),"documents":{"actual_app_saves":len(rows),"exact_original_reopens":6,"leaf_record_types":15,"negative_controls":5},
          "fresh_source_acquisitions":0,"result":"pass"}
(out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps(result))
