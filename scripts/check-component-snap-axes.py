#!/usr/bin/env python3
"""Read frozen original context-axis results with independently-built native protocol."""
import os,json,subprocess,hashlib,copy
from pathlib import Path
from acceptance_context import context
source=json.loads(Path("docs/audits/component-snap-axes-source.json").read_text());cases=source["cases"]
subprocess.run(["cargo","build","--locked","-p","ingetrazo-core","--example","context_screen_snap_fixture"],check=True)
binary=(Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/context_screen_snap_fixture").resolve()
def run(rows):return json.loads(subprocess.check_output([str(binary)],input=json.dumps(rows).encode()))
def matches(a,b):return a["kind"]==b["kind"] and a["axis"]==b["axis"] and len(b["point"])==3 and max(abs(x-y) for x,y in zip(a["point"],b["point"]))<4e-6
native=run(cases);assert len(native)==len(cases);assert [r["name"] for r in native]==[r["name"] for r in cases]
failures=[]
for case,result in zip(cases,native):
 if not matches(case["original"],result):failures.append({"input":case,"native":result})
assert not failures,json.dumps(failures,indent=2)
controls=[]
for field in ("kind","point"):
 wrong=copy.deepcopy(native[1])
 if field=="kind":wrong[field]="none"
 else:wrong[field][0]+=0.1
 assert not matches(cases[1]["original"],wrong);controls.append(field+" mutation rejected")
wrong=copy.deepcopy(cases);[r.update(frame=None) for r in wrong];world_only=run(wrong);assert any(not matches(c["original"],r) for c,r in zip(cases,world_only));controls.append("missing context frame rejected")
assert len(native[:-1])!=len(cases);controls.append("missing case rejected")
Path("docs/audits/component-snap-axes-native.json").write_text(json.dumps({"provenance":context(),"source_provenance":source["provenance"],"scope":source["scope"],"native_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"passed":len(cases),"negative_controls":controls,"results":native},indent=2)+"\n")
print(f"Frozen source/native context snapping {len(cases)}/{len(cases)} PASS with4negativecontrols")
