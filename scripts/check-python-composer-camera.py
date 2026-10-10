#!/usr/bin/env python3
"""Actual original frame camera plus fresh native frame schema reopen."""
import os,sys,json,tempfile,subprocess,copy,shutil
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.camera import OrbitCamera
from core.saved_views import SavedView
from core.composition import MarcoVista,Composicion,apply_frame_camera
from formats import igz
def xyz(p):return [p.x(),p.y(),p.z()]
def transform(m,p):return [sum(m[col*4+row]*p[col]for col in range(4))for row in range(4)]
def depth_visible(view,projection,p):
 v=transform(view,p+[1.]);clip=transform(projection,v);return clip[3]>0 and -clip[3]<=clip[2]<=clip[3]
def compare(c,r):
 expected={"target":xyz(c.target),"yaw":c.yaw,"pitch":c.pitch,"distance":c.distance,"fov_deg":c.fov_deg,"aspect":c.aspect,"perspective":c.perspective,"two_point":c.two_point}
 fields={k:[v,r["camera"][k]]for k,v in expected.items()if (v!=r["camera"][k]if isinstance(v,bool)else max(abs(a-b)for a,b in zip(v,r["camera"][k]))>2e-5 if isinstance(v,list)else abs(v-r["camera"][k])>2e-5)}
 matrices={}
 for key,values in [("view_matrix",list(c.view_matrix().data())),("projection_matrix",list(c.projection_matrix().data())),("eye",xyz(c.eye()))]:
  native_values=list(r[key])
  if key=="projection_matrix":
   # Qt/OpenGL uses clip Z -1..1; glam RH uses 0..1.
   for col in range(4):native_values[col*4+2]=2*native_values[col*4+2]-native_values[col*4+3]
  err=max(abs(a-b)for a,b in zip(values,native_values))
  if err>(1e-4 if key=="view_matrix" else 2e-5):matrices[key]={"maximum_absolute_error":err,"original":values,"rust":r[key]}
 return fields,matrices
def main():
 app=QApplication([]);s=Scene();m=Mesh();m.add_face([V(0,0,0),V(2,0,0),V(2,4,2),V(0,4,2)])
 child=Group(m,"Child");child.xform=QMatrix4x4();child.xform.scale(-2,3,1);parent=Group(name="Parent");parent.xform=QMatrix4x4();parent.xform.translate(10,20,0);parent.children=[child]
 hidden=Group(m,"Hidden");hidden.hidden=True;hidden.xform=QMatrix4x4();hidden.xform.translate(1000,1000,1000);s.groups=[parent,hidden]
 s.saved_views=[SavedView("Saved",target=(11,22,1),distance=7,yaw=-1.2,pitch=.25,fov_deg=62,perspective=True,two_point=True)]
 cases=[("std-top",dict(view_key="std:top")),("std-bottom-roll",dict(view_key="std:bottom",rot_deg=90)),("std-iso",dict(view_key="std:iso")),("live-parallel",{}),("saved-parallel",dict(view_key="scene:Saved")),("saved-perspective",dict(view_key="scene:Saved",perspective=True)),("lens-high-roll",dict(view_key="std:front",perspective=True,cam_target=(4,5,6),cam_yaw=-1.2,cam_pitch=.3,cam_distance=7,cam_fov=200,rot_deg=90)),("lens-low-distance",dict(perspective=True,cam_fov=1,cam_distance=-1)),("fit-perspective",dict(view_key="std:iso",perspective=True)),("pixel-aspect",dict(view_key="std:top",w_mm=170,h_mm=150))]
 frames=[MarcoVista(w_mm=150,h_mm=150,scale_n=20,**kw)if name!="pixel-aspect" else MarcoVista(scale_n=20,**kw)for name,kw in cases];s.compositions=[Composicion(frames=frames)]
 rows=[];failures=[];original_cameras=[]
 with tempfile.TemporaryDirectory()as d:
  inp,out,native=Path(d)/"source.igz",Path(d)/"camera.json",Path(d)/"fresh.igz";igz.save_scene(s,inp)
  subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","composer_camera_acceptance_fixture","--",str(inp),str(out),str(native)],check=True);results=json.loads(out.read_text())
  shutil.copy2(inp,"docs/audits/composer-camera-source.igz");shutil.copy2(native,"docs/audits/composer-camera-fresh.igz");reopened=Scene();igz.load_into(reopened,native)
  assert len(reopened.compositions[0].frames)==len(frames)
  for i,((name,kw),r)in enumerate(zip(cases,results)):
   original=Scene();igz.load_into(original,inp);f=original.compositions[0].frames[i];c=OrbitCamera();c.yaw=-.9;c.pitch=.35;c.distance=9;c.fov_deg=52;c.target=V(2,3,4);c.perspective=True;c.two_point=False
   saved=original.saved_views[0]if f.view_key=="scene:Saved" else None;apply_frame_camera(c,f,saved,original)
   original_cameras.append(c);fields,matrices=compare(c,r)
   rf=reopened.compositions[0].frames[i]
   for key in ["cam_target","cam_yaw","cam_pitch","cam_distance","cam_fov","rot_deg","perspective","view_key"]:assert getattr(rf,key)==getattr(f,key),(name,key,getattr(rf,key),getattr(f,key))
   point=xyz(c.eye()-c.forward()*2.);expected_visible=depth_visible(list(c.view_matrix().data()),list(c.projection_matrix().data()),point)
   normalized_projection=list(r["projection_matrix"])
   for col in range(4):normalized_projection[col*4+2]=2*normalized_projection[col*4+2]-normalized_projection[col*4+3]
   native_visible=depth_visible(r["view_matrix"],normalized_projection,point)
   assert expected_visible==native_visible==(not c.perspective),(name,expected_visible,native_visible)
   same=not fields and not matrices;rows.append({"case":name,"camera_match":same,"native_fresh_fields_reopened":True,"behind_eye_depth_visible":native_visible})
   if not same:failures.append({"case":name,"fields":fields,"matrices":matrices})
 controls=[]
 for name,index,delta in [("changed-target",0,1.),("changed-lens",6,10.),("changed-roll-matrix",1,1.)]:
  changed=copy.deepcopy(results[index])
  if name=="changed-target":changed["camera"]["target"][0]+=delta
  elif name=="changed-lens":changed["camera"]["fov_deg"]+=delta
  else:changed["view_matrix"][0]+=delta
  fields,matrices=compare(original_cameras[index],changed);detected=bool(fields or matrices)
  assert detected;controls.append({"case":name,"detected":True})
 report={"negative_controls":controls,"provenance":context(),"passed":not failures,"cases":rows,"failures":failures,"scope":"Actual original apply_frame_camera on original authored IGZ versus Rust helper; transformed nested visible bounds, hidden distant group, current/saved/standard cameras, exact plans, roll, lens/distance clamps, scale and render-pixel aspect. View matrix compared at1e-4, projection/eye at2e-5 Qt float32 tolerance. Clip Z explicitly maps glam0..1 to Qt/OpenGL−1..1; up basis judged through view matrix, allowing equivalent Gram-Schmidt up vectors. Native fresh frame fields reopened in original. Renderer pixels, annotations, full composer interactions remain separate."}
 Path("docs/audits/python-composer-camera-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(rows));assert not failures,[f["case"]for f in failures]
if __name__=="__main__":main()
