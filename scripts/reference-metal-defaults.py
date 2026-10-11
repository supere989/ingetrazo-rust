#!/usr/bin/env python3
"""Frozen original scalar/soft-plane controls; no GPU/presentation claim."""
import ast, hashlib, json, pathlib, struct, sys, types
import numpy as np
from PySide6.QtGui import QGuiApplication,QMatrix4x4,QVector3D,QVector4D
sys.path.insert(0,"/work/ingetrazo")
from core.camera import OrbitCamera
from core.scene import Scene
from core.mesh import Mesh
app=QGuiApplication.instance() or QGuiApplication([])
p=pathlib.Path("/work/ingetrazo/views/viewport.py");tree=ast.parse(p.read_text());cls=next(x for x in tree.body if isinstance(x,ast.ClassDef) and any(isinstance(f,ast.FunctionDef) and f.name=="_sky_gradient" for f in x.body))
names={"_horizon_ndc_y","_sky_gradient","_upload_silhouette_edges"};nodes=[x for x in cls.body if isinstance(x,ast.FunctionDef) and x.name in names]
ns={"QVector3D":QVector3D,"QVector4D":QVector4D,"_cache_ver":lambda self:1}
exec(compile(ast.Module(body=nodes,type_ignores=[]),str(p),"exec"),ns)
class Stub:
 _SKY_GRAD_SPAN=1.4;_GROUND_GRAD_SPAN=.9
 _horizon_ndc_y=ns["_horizon_ndc_y"]
 _sky_gradient=staticmethod(ns["_sky_gradient"].__func__)
 _upload_silhouette_edges=ns["_upload_silhouette_edges"]
 def _placements(self):return []
 def _placements_epoch(self):return 1
class Vbo:
 def bind(self):pass
 def release(self):pass
 def allocate(self,raw,size=None):self.raw=raw if isinstance(raw,bytes) else b""
ns["Viewport"]=Stub
sky=[]
for perspective in [True,False]:
 for pitch in [-1.3,-.25,0.,.3,1.3]:
  v=Stub();v.camera=OrbitCamera();v.camera.pitch=pitch;v.camera.perspective=perspective
  m=v.camera.projection_matrix()*v.camera.view_matrix();eye=v.camera.eye();hy=v._horizon_ndc_y(m)
  colors=v._sky_gradient((.925,.935,.945),(.815,.820,.815),hy)
  sky.append(dict(eye=list(eye.toTuple()),target=list(v.camera.target.toTuple()),mvp=list(m.data()),horizon=hy,colors_bits=[[struct.unpack("<I",struct.pack("<f",c))[0] for c in row] for row in colors]))
profiles=[]
for eye in [(0,2,2),(0,-2,2),(0,2,-2),(0,-2,-2),(0,0,0)]:
 s=Scene();s.loose_mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])
 s.loose_mesh.add_face([QVector3D(1,0,0),QVector3D(0,0,0),QVector3D(0,0,1)])
 for e in s.loose_mesh.edges:e.soft=len(e.faces)==2
 v=Stub();v.scene=s;v.camera=types.SimpleNamespace(eye=lambda:QVector3D(*eye),projection_matrix=lambda:QMatrix4x4(),view_matrix=lambda:QMatrix4x4());v._silhouette_vbo=Vbo()
 count=v._upload_silhouette_edges();raw=v._silhouette_vbo.raw
 profiles.append(dict(eye=eye,count=count,positions=list(struct.unpack("<"+"f"*(len(raw)//4),raw))))
group_vertices=[[0,0,0],[.5,0,0],[1,0,0],[0,1,0],[0,0,1]]
group_faces=[[0,1,2,3],[2,1,4]]
m=Mesh()
for face in group_faces:m.add_face([QVector3D(*group_vertices[i]) for i in face])
group_planes=[dict(normal=list(f.normal().toTuple()),centroid=list(f.centroid().toTuple())) for f in m.faces]
result=dict(group=dict(vertices=group_vertices,faces=group_faces,planes=group_planes),source="6be29fe437a7cd992d4e079f71720821b25216b9",source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),helper_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),sky=sky,profiles=profiles)
pathlib.Path(sys.argv[1]).write_text(json.dumps(result,indent=2)+"\n")
