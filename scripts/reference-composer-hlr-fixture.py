#!/usr/bin/env python3
"""Freeze actual pinned Python HLR outputs and corresponding native input scenes."""
import argparse,json,os,sys,subprocess
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
p=argparse.ArgumentParser();p.add_argument("--original",type=Path,default=Path("/work/ingetrazo"));p.add_argument("--output",type=Path,required=True);args=p.parse_args();sys.path.insert(0,str(args.original.resolve()))
from PySide6.QtGui import QVector3D
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.camera import OrbitCamera
from core.composition import MarcoVista,apply_frame_camera
from core.section import SectionPlane
from core.hlr import hlr_drawing
from formats.igz import save_scene
from tests.test_hlr_hidden_lines import _scene_with_a_box_behind_a_wall
from tests.test_hlr_section_cap import _box,_cut_at_y
app=QApplication.instance() or QApplication([])
args.output.mkdir(parents=True,exist_ok=True)
cases=[]
cases.append(("wall_hidden",_scene_with_a_box_behind_a_wall(),"std:front"))
s=Scene();_box(s.mesh,0,0,0,4,6,4);_box(s.mesh,1,3,1,3,5,3);_cut_at_y(s,2.);cases.append(("section_board",s,"std:front"))
s=Scene();_box(s.mesh,0,0,0,4,6,4);_box(s.mesh,1,0,1,3,6,3);_cut_at_y(s,2.);cases.append(("hollow_cap",s,"std:front"))
for view in ["top","iso","front"]:
 s=Scene();_box(s.mesh,0,0,0,1,1,1);cases.append((view+"_box",s,"std:"+view))
s=Scene();s.mesh.add_face([QVector3D(-1,-1,2),QVector3D(1,-1,2),QVector3D(1,1,2),QVector3D(-1,1,2)]);s.mesh.add_edge(QVector3D(-2,0,1),QVector3D(2,0,1));cases.append(("partial_rear_line",s,"std:top"))
s=Scene();s.mesh.add_face([QVector3D(0,0,0),QVector3D(2,0,0),QVector3D(2,0,3),QVector3D(0,0,3)]);sp=SectionPlane(QVector3D(0,0,1.5),QVector3D(0,0,1));s.section_planes.append(sp);s.set_active_section(sp);cases.append(("open_cut_chain",s,"std:top"))
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer
from PySide6.QtGui import QMatrix4x4
import math
s=Scene();m=Mesh();_box(m,0,0,0,2,3,4);child=Group(m,"Child");child.xform=QMatrix4x4();child.xform.scale(-2,3,.5);parent=Group(name="Parent");parent.xform=QMatrix4x4();parent.xform.translate(7,11,2);parent.xform.rotate(27,QVector3D(0,0,1));parent.children=[child];s.groups=[parent];cases.append(("nested_reflected_box",s,"std:iso"))
s=Scene();_box(s.mesh,0,0,0,2,3,4);s.layers.append(Layer("Hidden",visible=False));m=Mesh();_box(m,10,0,0,12,3,4);g=Group(m,"HiddenGroup");g.layer="Hidden";s.groups=[g];cases.append(("hidden_layer_group",s,"std:front"))
s=Scene();_box(s.mesh,0,0,0,2,3,4);s.layers.append(Layer("Hidden",visible=False));f=s.mesh.add_face([QVector3D(10,0,0),QVector3D(12,0,0),QVector3D(12,0,4),QVector3D(10,0,4)]);f.attrs["layer"]="Hidden"
for e in s.mesh.edges:
 if e.a.x()>9 and e.b.x()>9:e.layer="Hidden"
cases.append(("hidden_layer_face",s,"std:front"))
s=Scene();n=16;bottom=[QVector3D(math.cos(i*2*math.pi/n),math.sin(i*2*math.pi/n),0)for i in range(n)];top=[QVector3D(p.x(),p.y(),3)for p in bottom];s.mesh.add_face(list(reversed(bottom)));s.mesh.add_face(top)
for i in range(n):j=(i+1)%n;s.mesh.add_face([bottom[i],bottom[j],top[j],top[i]])
for e in s.mesh.edges:
 if abs(e.a.z()-e.b.z())>2:e.soft=True
cases.append(("soft_cylinder_silhouette",s,"std:iso"))
s2=Scene();g=Group(s.mesh,"ReflectedSoft");g.xform=QMatrix4x4();g.xform.translate(7,11,2);g.xform.rotate(27,QVector3D(0,0,1));g.xform.scale(-2,3,.5);s2.groups=[g];cases.append(("soft_reflected_silhouette",s2,"std:iso"))
summary=[]
for name,scene,key in cases:
 frame=MarcoVista(view_key=key,scale_n=100.,w_mm=100.,h_mm=100.);camera=OrbitCamera();apply_frame_camera(camera,frame,scene=scene)
 camera_value={"target":[camera.target.x(),camera.target.y(),camera.target.z()],"distance":camera.distance,"yaw":camera.yaw,"pitch":camera.pitch,"up":[camera.up.x(),camera.up.y(),camera.up.z()],"fov_deg":camera.fov_deg,"aspect":camera.aspect,"perspective":camera.perspective,"two_point":camera.two_point,"znear":camera.znear,"zfar":camera.zfar}
 save_scene(scene,args.output/(name+".igz"))
 reference={"name":name,"source_commit":subprocess.check_output(["git","-C",str(args.original),"rev-parse","HEAD"],text=True).strip(),"camera":camera_value,"drawings":{}}
 for hidden in [False,True]:
  d=hlr_drawing(scene,camera,hidden=hidden);reference["drawings"][str(hidden).lower()]={"world":d.world.tolist(),"segs":d.segs.tolist(),"kinds":d.kinds.tolist(),"loops":[r.tolist() for r in d.loops]}
 (args.output/(name+".reference.json")).write_text(json.dumps(reference,indent=2)+"\n")
 summary.append({"case":name,"visible_segments":len(reference["drawings"]["false"]["world"]),"with_hidden_segments":len(reference["drawings"]["true"]["world"])})
print(json.dumps(summary,indent=2))
