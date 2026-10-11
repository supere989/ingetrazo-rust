#!/usr/bin/env python3
import json,sys,os,copy
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.camera import OrbitCamera
from formats import igz
app=QApplication.instance() or QApplication([])
def mask(gs):
 out=[]
 for g in gs:out.append(g.hidden);out.extend(mask(g.children))
 return out
def flip(gs):
 for g in gs:g.hidden=not g.hidden;flip(g.children)
def validate(expected,observed):
 if set(expected)!=set(observed):raise AssertionError("case inventory")
 for name in expected:
  if expected[name]!=observed[name]:raise AssertionError(name)
def recall(path,expected):
 s=Scene();igz.load_into(s,path);flip(s.groups);s.saved_views[-1].apply(s,OrbitCamera())
 if mask(s.groups)!=expected:raise AssertionError(str(path))
 return s
d=Path(sys.argv[1])
# Only the referenced parent receives an ID on this unchanged legacy tree.
captured=json.loads((d/"legacy-captured-native.igz").read_text())["scene"]["groups"][0]
assert captured["uid"] and "uid" not in captured["children"][0]
expected=json.loads((d/"original.json").read_text());observed=json.loads((d/"native.json").read_text());validate(expected,observed)
for name,data in expected.items():
 recall(d/(name+"-captured-native.igz"),data["apply_mask"] if name!="duplicate" else [False,True,False])
 s=recall(d/(name+"-native.igz"),[False,False,True] if name=="duplicate" else data["updated_mask"])
 raw=json.loads((d/(name+"-native.igz")).read_text())["scene"]
 assert raw["saved_views"][-1]["vendor_view"]=={"untouched":[1,2,3]}
 def groups(gs):
  for g in gs:yield g;yield from groups(g.get("children",[]))
 if name!="legacy":assert raw["saved_views"][0]["vendor_original"]=={"retain":"literal"}
 original=json.loads((d/(name+".igz")).read_text())["scene"]
 for a,b in zip(groups(original["groups"]),groups(raw["groups"])):
  assert a["xform"]==b["xform"]
  if a.get("uid"):assert a["uid"]==b["uid"]
  if "vendor_group" in a:assert a["vendor_group"]==b["vendor_group"]
  if "proto" in a:assert a["proto"]==b["proto"]
 for g in groups(raw["groups"]):assert g["uid"]
 # Every captured generated reference resolves after actual Python reopen.
 if name=="legacy":assert set(s.saved_views[-1].hidden_objects)-{"future-placement"}<=set(s.groups_by_uid())
negatives=0
for name in expected:
 bad=copy.deepcopy(observed);bad[name]["apply_mask"][0]=not bad[name]["apply_mask"][0]
 try:validate(expected,bad)
 except AssertionError:negatives+=1
 else:raise AssertionError("accepted mutation")
# Literal old serializer: deleting generated IDs breaks recall after Python load.
p=d/"legacy-captured-native.igz";raw=json.loads(p.read_text())
def strip(gs):
 for g in gs:g.pop("uid",None);strip(g.get("children",[]))
strip(raw["scene"]["groups"]);old=d/"legacy-old-missing-uid.igz";old.write_text(json.dumps(raw))
try:recall(old,expected["legacy"]["apply_mask"])
except AssertionError:negatives+=1
else:raise AssertionError("old missing-uid accepted")
# Mutating an authored literal UID without remapping its view must fail actual recall.
raw=json.loads((d/"literal-captured-native.igz").read_text());raw["scene"]["groups"][0]["uid"]="rewritten-runtime-key";bad=d/"literal-old-rewritten-uid.igz";bad.write_text(json.dumps(raw))
try:recall(bad,expected["literal"]["apply_mask"])
except AssertionError:negatives+=1
else:raise AssertionError("lossy literal-ID rewrite accepted")
print(json.dumps({"cases":len(expected),"negative_controls":negatives,"actual_python_recall":2*len(expected)}))
