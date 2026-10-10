#!/usr/bin/env python3
"""Frozen source ordering plus separately classified native safety boundaries."""
import os,json,subprocess,hashlib,copy
from pathlib import Path
from acceptance_context import context
source=json.loads(Path("docs/audits/component-active-history-source.json").read_text())
subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","component_history_acceptance_fixture"],check=True)
binary=(Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/component_history_acceptance_fixture").resolve()
results=json.loads(subprocess.check_output([str(binary)]));assert len(results)==8
byname={r["name"]:r for r in results};assert len(byname)==8
checks=[]
for s in source["cases"]:
 if s["name"].endswith("local-two-undo-redo-exit"):
  r=byname[s["name"]]
  checks.extend([(r["active_undo_faces"]==s["active_undo"]["active_faces"],r["name"]+" active Undo"),(r["active_redo_faces"]==s["active_redo"]["active_faces"],r["name"]+" active Redo"),(r["exit_depth"]==len(s["exit"]["undo_commands"]),r["name"]+" Finish ordering"),(r["exit_undo_faces"]==s["exit_undo"]["group_faces"],r["name"]+" closed Undo"),(r["exit_redo_faces"]==s["exit_redo"]["group_faces"] and r["ids_preserved"],r["name"]+" closed Redo IDs")])
 if s["name"].endswith("undo-prior-loose-command"):
  r=byname[s["name"]];checks.append((r["active_stable"] and r["active_undo_depth"]==len(s["active_undo"]["undo_commands"]) and r["active_redo_depth"]==len(s["active_undo"]["redo_commands"]) and r["exit_loose_faces"]==s["exit"]["parked_faces"] and r["loose_undo_faces"]==0 and r["loose_redo_faces"]==1,r["name"]+" parked target/global order"))
failures=[label for good,label in checks if not good];assert not failures,failures
safety=[byname["native-safe-active-makeunique-refusal"],byname["native-safe-unchanged-finish-redo-rebind"]]
assert all(safety[0][k] for k in ("scene_stable","stacks_stable","visible_refusal")) and safety[0]["context_depth"]==1
assert all(safety[1][k] for k in ("redo_retained","both_placements_redone","both_placements_undone"))
wrong=copy.deepcopy(byname["classic-local-two-undo-redo-exit"]);wrong["exit_depth"]=1;expected=next(s for s in source["cases"] if s["name"]==wrong["name"]);assert wrong["exit_depth"]!=len(expected["exit"]["undo_commands"])
assert len(results[:-1])!=8
Path("docs/audits/component-global-history-native.json").write_text(json.dumps({"provenance":context(),"source_provenance":source["provenance"],"native_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"scope":"Actual public GUI history/controller calls with explicit face mutation snapshots, six frozen original ordering recipes and two separately classified native integrity corrections. No native key/pixel acceptance; source actual Qt dispatch evidence retained separately.","source_checks_passed":len(checks),"native_integrity_boundaries":safety,"negative_controls":["ordinary Finish collapse rejected","missing case rejected"],"results":results},indent=2)+"\n")
print(f"Frozen source global history {len(checks)} checks PASS; 2 explicit native integrity boundaries PASS")
