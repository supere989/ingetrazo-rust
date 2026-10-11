#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.camera import OrbitCamera
from core.saved_views import SavedView
from formats import igz
app=QApplication.instance() or QApplication([])
d=Path(sys.argv[1]);d.mkdir(parents=True,exist_ok=True)
I=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
def group(uid,hidden=False,children=None):
 g={"name":"placement","component":False,"proto":0,"xform":I,"hidden":hidden,"vendor_group":{"keep":7}}
 if uid is not None:g["uid"]=uid
 if children:g["children"]=children
 return g
cases={"literal":[group("0123456789abcdef",True,[group("nested/child",True)]),group("1234567890abcdef1234567890abcdef"),group("12345678-90ab-cdef-1234-567890abcdef",True)],"legacy":[group(None,True,[group(None,False)])],"duplicate":[group("same",True),group("other",True),group("same",False)]}
cases["generated"]=[group(None,False,[group(None,True)])]
cases["literal"][0]["xform"]=[-1,0,0,0,0,1,0,0,0,0,1,0,3,4,5,1]
cases["literal"][0]["children"][0]["xform"]=[1,0,0,0,0,1,0,0,0,0,1,0,1,2,3,1]
def mask(gs):
 out=[]
 for g in gs:out.append(g.hidden);out.extend(mask(g.children))
 return out
def flip(gs):
 for g in gs:g.hidden=not g.hidden;flip(g.children)
report={}
for name,groups in cases.items():
 path=d/(name+".igz");path.write_text(json.dumps({"igz_format":1,"app_version":"source-fixture","scene":{"edges":[],"faces":[],"protos":[{"edges":[],"faces":[]}],"groups":groups,"saved_views":[]}}))
 s=Scene();igz.load_into(s,path);
 if name=="generated":igz.save_scene(s,path)
 c=OrbitCamera();v=SavedView.capture("capture",s,c);capture=list(v.hidden_objects)
 if name!="legacy":
  raw=json.loads(path.read_text());raw["scene"]["saved_views"]=[v.to_dict()];raw["scene"]["saved_views"][0]["vendor_original"]={"retain":"literal"};path.write_text(json.dumps(raw))
 flip(s.groups);v.apply(s,c);applied=mask(s.groups)
 flip(s.groups);v.recapture(s,c);updated=mask(s.groups);v.hidden_objects.append("future-placement");s.saved_views=[v];igz.save_scene(s,d/(name+"-source.igz"))
 report[name]={"capture_ids":None if name=="legacy" else capture,"apply_mask":applied,"updated_mask":updated,"updated_ids":None if name=="legacy" else list(v.hidden_objects)}
(d/"original.json").write_text(json.dumps(report,indent=2)+"\n")
