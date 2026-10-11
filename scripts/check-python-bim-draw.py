#!/usr/bin/env python3
"""Source AutoTagDrawnFacesCommand declaration ownership, independent exact observer."""
import copy,hashlib,json,os,subprocess,tempfile,types,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History,AddFaceCommand,AutoTagDrawnFacesCommand
from core.edits import build_add_edge,build_add_edges
from tools.arc import commit_arc
from acceptance_context import context

def xyz(p):return [p.x(),p.y(),p.z()]
def ring(points):
 p=[tuple(v)for v in points]
 return min(tuple(p[i:]+p[:i])for i in range(len(p)))
def signature(result):
 return sorted((ring(f["vertices"]),tuple(sorted(ring(h)for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True))for f in result["faces"])
def bim_signature(result):
 r=copy.deepcopy(result)
 for f in r["faces"]:f["attrs"]={"ifc":f["attrs"]["ifc"]}if "ifc"in f["attrs"]else{}
 return signature(r)
def main():
 base=json.loads(Path("docs/audits/drafting-expanded-corpus.json").read_text())
 names=["painted-line-chord","line-bowtie","rectangle-band","rectangle-overlap","freehand-closed","freehand-open-chord","old-curve-global-rebuild","holed-line-chord","holed-rectangle-band","holed-freehand-chord","holed-outside-void-chord","two-hole-chord-locality","holed-sequential-chords","old-curve-holed-chord"]
 cases=[copy.deepcopy(next(c for c in base if c["name"]==name))for name in names]
 for c in cases:c["active"]={"class":"IfcWall","name":"Muros N1"}
 inherited=copy.deepcopy(cases[0]);inherited["name"]="inherited-chord";inherited["faces"][0]["attrs"]["ifc"]={"class":"IfcSlab","name":"existing","id":17};cases.append(inherited)
 rect=[[0,0,0],[2,0,0],[2,1,0],[0,1,0]]
 def op(r):return {"segments":[[a,b]for a,b in zip(r,r[1:]+r[:1])],"explicit":r,"curve":None}
 cases.append({"name":"two-traces","faces":[],"edges":[],"active":{"class":"IfcWall","name":"Muros N1"},"ops":[op(rect),op([[x,y+5,z]for x,y,z in rect])]})
 off=copy.deepcopy(cases[-1]);off["name"]="inactive-traces";del off["active"];cases.append(off)
 for index,name in [(4,"nonflat-freehand-closed"),(5,"nonflat-freehand-chord")]:
  c=copy.deepcopy(cases[index]);c["name"]=name;c["edges"].append({"points":[[10,0,4],[11,0,4]],"curve":None});cases.append(c)
 closed=copy.deepcopy(cases[15]);closed["name"]="line-closing-cycle";closed["ops"]=[dict(op(rect),explicit=None)];cases.append(closed)
 inner=[[2,2,0],[3,2,0],[3,3,0],[2,3,0]]
 c={"name":"contained-authored-footprint","faces":[{"vertices":[[0,0,0],[6,0,0],[6,6,0],[0,6,0]],"attrs":{"vendor":71,"color":[1,0,0]}}],"edges":[],"active":{"class":"IfcWall","name":"Muros N1"},"ops":[op(inner)]};cases.append(c)
 c=copy.deepcopy(c);c["name"]="contained-tagged-mother";c["faces"][0]["attrs"]["ifc"]={"id":17,"class":"IfcSlab","name":"existing"};cases.append(c)
 out=Path("docs/audits/bim-draw");out.mkdir(exist_ok=True)
 (out/"corpus.json").write_text(json.dumps(cases,indent=2)+"\n")
 target=Path(os.environ.get("CARGO_TARGET_DIR","target"))
 subprocess.run(["cargo","build","--locked","-p","ingetrazo-core","--example","bim_draw_acceptance_fixture","-j2"],check=True)
 exe=target/"debug/examples/bim_draw_acceptance_fixture"
 with tempfile.TemporaryDirectory(prefix="bim-draw-")as tmp:
  native=Path(tmp)/"native.json";subprocess.run([str(exe),str(out/"corpus.json"),str(native)],check=True);results=json.loads(native.read_text())
 rows=[];frozen=[]
 assert len(results)==len(cases)==22
 assert subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
 for c,r in zip(cases,results):
  s=Scene();s.active_ifc=c.get("active")
  for f in c["faces"]:s.mesh.add_face([V(*p)for p in f["vertices"]],hole_loops=[[V(*p)for p in h]for h in f.get("holes",[])]).attrs.update(copy.deepcopy(f["attrs"]))
  for e in c["edges"]:s.mesh.add_edge(V(*e["points"][0]),V(*e["points"][1])).curve=e["curve"]
  h=History(s)
  for o in c["ops"]:
   seg=[(V(*a),V(*b))for a,b in o["segments"]]
   if o["curve"]is not None:commit_arc(types.SimpleNamespace(scene=s,history=h),[seg[0][0]]+[b for a,b in seg])
   elif o["explicit"]is not None:h.execute(build_add_edges(s,seg,detect_faces=False,extra=[AddFaceCommand([V(*p)for p in o["explicit"]])]))
   else:
    for a,b in seg:h.execute(build_add_edge(s,a,b))
   assert h.last_error is None,h.last_error
  original={"faces":[{"vertices":[xyz(p)for p in f.vertices],"holes":[[xyz(p)for p in hole]for hole in f.holes],"attrs":f.attrs}for f in s.mesh.faces]}
  rows.append({"case":c["name"],"match":signature(original)==signature(r),"bim_match":bim_signature(original)==bim_signature(r),"source_faces":len(original["faces"]),"native_faces":len(r["faces"])})
  frozen.append({"case":c["name"],"original":original,"native":r})
 truthiness=[]
 for value in [None,False,0,0.0,"",[],{},True,1,"opaque",[17],{"id":True,"vendor":71}]:
  scene=Scene();face=scene.mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);face.attrs["ifc"]=copy.deepcopy(value);before=copy.deepcopy(face.attrs)
  try:AutoTagDrawnFacesCommand([types.SimpleNamespace(face=face)],{"class":"IfcWall","name":"Wall"}).do(scene);refused=False
  except AttributeError:refused=True;assert face.attrs==before
  assert refused==(bool(value)and not isinstance(value,dict))
  if not refused:assert face.attrs["ifc"]==(value if value else{"class":"IfcWall","name":"Wall","id":1})
  truthiness.append({"input":value,"source_refused":refused,"after":face.attrs})
 (out/"source-truthiness.json").write_text(json.dumps(truthiness,indent=2)+"\n")
 controls=[]
 for name,i,edit in [("drop-authored-tag",0,lambda f:f[0]["attrs"].pop("ifc")),("retag-neighbor",6,lambda f:f[0]["attrs"].update({"ifc":{"id":991}})),("collapse-trace-identity",15,lambda f:[x["attrs"]["ifc"].update({"id":1})for x in f])]:
  changed=copy.deepcopy(results[i]);edit(changed["faces"]);assert signature(changed)!=signature(results[i]);controls.append(name)
 report={"bim_contract_passed":all(r["bim_match"]for r in rows),"full_attributes_passed":all(r["match"]for r in rows),"cases":rows,"negative_controls":controls,"source":context(),"binary_sha256":hashlib.sha256(exe.read_bytes()).hexdigest(),"scope":"Exact oriented authored rings/holes and all attributes under original active tagging command. Runtime app history and actual UI delivery have separate evidence."}
 (out/"results.json").write_text(json.dumps(frozen,indent=2)+"\n");(out/"report.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(rows));assert report["bim_contract_passed"]
 assert all(r["match"]for r in rows[:17])
if __name__=="__main__":main()
