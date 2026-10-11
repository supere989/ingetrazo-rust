#!/usr/bin/env python3
"""Independent frozen original/world-frame comparison and adversarial negatives."""
import argparse,json,copy,subprocess,os,hashlib
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument("--prebuilt-dir",type=Path,help="Fresh native debug example executables built by the caller")
args=parser.parse_args()
if os.environ.get("CARGO_BUILD_TARGET") or os.environ.get("CARGO_PROFILE"):
    parser.error("Component acceptance supports native debug executables only; unset CARGO_BUILD_TARGET/CARGO_PROFILE")
if args.prebuilt_dir is None:
    target=Path(os.environ.get("CARGO_TARGET_DIR","target")).resolve()
    build=subprocess.run(["cargo","build","--locked","--message-format=json-render-diagnostics","-j2","--target-dir",str(target),"-p","ingetrazo-gui","--example","component_ci_world_bake_fixture"],check=True,stdout=subprocess.PIPE,text=True)
    binary_dir=target/"debug"/"examples"
    artifacts=set()
    for line in build.stdout.splitlines():
        message=json.loads(line)
        if message.get("reason")=="compiler-artifact" and message.get("executable") and message.get("target",{}).get("name")=="component_ci_world_bake_fixture" and "example" in message["target"].get("kind",[]):
            artifacts.add(Path(message["executable"]).resolve())
    expected=(binary_dir/("component_ci_world_bake_fixture"+(".exe" if os.name=="nt" else ""))).resolve()
    if artifacts!={expected}:
        parser.error(f"Unsupported target configuration or missing Cargo artifact: {sorted(map(str,artifacts))}; expected {expected}")
else:
    binary_dir=args.prebuilt_dir.resolve()
binary=binary_dir/("component_ci_world_bake_fixture"+(".exe" if os.name=="nt" else ""))
if not binary.is_file() or not os.access(binary,os.X_OK):
    parser.error(f"Required prebuilt executable missing or not executable: {binary}")
d=Path("docs/audits/component-ci-world-bake")
subprocess.run([str(binary),str(d)],check=True)
source=json.loads((d/"original.json").read_text())["cases"];native=json.loads((d/"native.json").read_text())
def equal(a,b):
 if isinstance(a,(int,float))and not isinstance(a,bool):return a==b
 if isinstance(a,list):return len(a)==len(b)and all(equal(x,y)for x,y in zip(a,b))
 if isinstance(a,dict):return all(k in b and equal(v,b[k])for k,v in a.items())
 return a==b
def history_contract(row):
 h=row["history_presentation"]
 return h["selection_empty_after_commit_undo_redo"] is True and h["initial_version"]<=h["committed_version"]<h["undo_version"]<h["redo_version"]
assert all(history_contract(s) for s in source)
def accepts(rows):return all(equal({k:s[k]for k in ["name","parent_drawing_frame","child_drawing_frame","entry_world_points","authored_world_point","committed","undone"]},n)for s,n in zip(source,rows))and len(rows)==3 and all(history_contract(n) for n in rows)
assert accepts(native)
negatives={}
for mutation in ["preserve_parent_transform","reverse_child_matrix_order","local_authoring"]:
 bad=copy.deepcopy(native)
 if mutation=="preserve_parent_transform":bad[0]["committed"]["selected"]=source[0]["before"]["selected"]
 if mutation=="reverse_child_matrix_order":
  parent=source[1]["before"]["selected"];child=source[1]["before"]["child"]
  bad[1]["committed"]["child"]=[sum(child[row+k*4]*parent[k+col*4]for k in range(4))for col in range(4)for row in range(4)]
 if mutation=="local_authoring":bad[2]["authored_world_point"]=[0,0,5]
 assert not accepts(bad);negatives[mutation]="rejected"
tiny=copy.deepcopy(native); tiny[0]["authored_world_point"][0]+=2**-22; assert not accepts(tiny); negatives["sub_micro_coordinate_change"]="rejected"
for mutation in ["restore_selection", "restore_old_version"]:
 bad=copy.deepcopy(native)
 if mutation=="restore_selection":bad[0]["history_presentation"]["selection_empty_after_commit_undo_redo"]=False
 else:bad[0]["history_presentation"]["undo_version"]=bad[0]["history_presentation"]["initial_version"]
 assert not accepts(bad);negatives[mutation]="rejected"
report={"passed":True,"cases":3,"negative_controls":negatives,"precision":"Exact source/native numeric equality for these binary-representable affine controls; flags/counts/IDs remain exact. Arbitrary float32/f64 rotated precision remains separate open scope.","scope":"Actual original Viewport controller versus native component controller under translated, rotated and tilted/scaled frames; no rendered gesture proof.","source_sha256":hashlib.sha256((d/"original.json").read_bytes()).hexdigest()};(d/"comparison.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report))
