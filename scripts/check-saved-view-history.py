#!/usr/bin/env python3
"""Strict operative controller history comparison; all mutations use validate."""
import copy,json,sys
from pathlib import Path
LABELS="geometry-apply geometry-undo geometry-redo update-rename-apply updated-geometry-undo updated-geometry-redo hide-then-recall hide-undo hide-redo hide-second-undo hide-second-redo already-hidden-recall already-hidden-undo already-hidden-redo already-hidden-later-recall-undo already-hidden-later-redo unhide-recall unhide-undo unhide-redo unhide-second-undo delete-recall delete-undo delete-redo manual-camera-undo manual-camera-redo panel-delete-undo panel-delete-redo nested-context-camera nested-context-camera-undo nested-context-camera-redo".split()
FIELDS={"label","groups","distance","target","edges","vertices","edge_endpoints","views","undo","redo"}
def validate(expected,actual):
 assert isinstance(expected,list) and isinstance(actual,list)
 assert [s.get("label") for s in expected]==LABELS
 assert [s.get("label") for s in actual]==LABELS
 for e,a in zip(expected,actual):
  assert set(e)==FIELDS and set(a)==FIELDS
  assert e==a,(e["label"],e,a)
def reject(expected,actual):
 try:validate(expected,actual)
 except (AssertionError,TypeError,KeyError):return
 raise AssertionError("negative accepted")
p=Path(sys.argv[1]);count=0
for case in ("unique","duplicate"):
 e=json.loads((p/case/"original.json").read_text());a=json.loads((p/case/"native.json").read_text());validate(e,a)
 for index,field,value in [(1,"distance",11),(1,"vertices",[]),(7,"groups",[]),(24,"target",[1,2,3]),(25,"views",["updated","visible"]),(28,"distance",20),(29,"edge_endpoints",[])]:
  m=copy.deepcopy(a);m[index][field]=value;reject(e,m);count+=1
 for m in (a[:-1],a+[a[-1]],list(reversed(a))):reject(e,m);count+=1
if (p/"baseline-native.json").exists():
 old=json.loads((p/"baseline-native.json").read_text());reject(e,old);count+=1
 # A genuine old candidate also disagrees on the original common fields,
 # independently of new raw-vertex fields and inventory expansion.
 baseline=json.loads((p/"baseline-original.json").read_text())
 assert any(x!=y for x,y in zip(baseline,old))
print(json.dumps({"cases":2,"exact_states":60,"rejected_controls":count}))
