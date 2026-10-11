#!/usr/bin/env python3
"""Compare actual pending Paste save traces and source reopens, with controls."""
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/"original.json").read_text());native=json.loads((p/"native.json").read_text());reopened=json.loads((p/"reopened.json").read_text())
cases=[s+"-"+f for s in ["save","save_as","save_as_cancel","failed"] for f in ["continue","cancel"]]
saves={s+"-"+f for s in ["save","save_as"] for f in ["continue","cancel"]}
def check(n,r,expected=source):
 for trace in [n,expected]:
  assert [x["case"] for x in trace["rows"]]==cases,"workflow inventory"
  for row in trace["rows"]:
   finish=row["case"].rsplit("-",1)[1]
   assert [x["phase"] for x in row["phases"]]==(["after","continue","undo","redo"] if finish=="continue" else ["after","cancel"]),"phase inventory"
   assert row["phases"][0]["pending"] is True,"pending Paste missing"
 assert n["rows"]==expected["rows"],"source/native state transition mismatch"
 assert n["invalid_refusal"] is True and n["immutable_preview_owner"] is True
 assert set(r)==saves,"reopen inventory"
 for row in expected["rows"]:
  if row["case"] in saves:assert r[row["case"]]==row["phases"][0]["state"],"saved whole-document geometry mismatch"
check(native,reopened);controls=[]
for kind in ["lost_preview","moved_preview","extra_history","fragment_save","missing_workflow"]:
 n=copy.deepcopy(native);r=copy.deepcopy(reopened)
 if kind=="lost_preview":n["rows"][0]["phases"][0]["pending"]=False
 elif kind=="moved_preview":n["rows"][0]["phases"][0]["offset"][0]+=1
 elif kind=="extra_history":n["rows"][0]["phases"][1]["undo"]+=1
 elif kind=="fragment_save":r["save-continue"]["groups"]=[]
 else:n["rows"].pop()
 try:check(n,r)
 except AssertionError:controls.append(kind)
 else:raise AssertionError("undetected control "+kind)
print(json.dumps({"passed":True,"workflows":8,"source_states":24,"original_reopens":4,"invalid_finish_preserves_pending_owner":True,"immutable_preview_owner":True,"negative_controls":controls}))
