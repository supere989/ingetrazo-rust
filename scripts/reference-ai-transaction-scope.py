#!/usr/bin/env python3
"""Offline operative source ownership control; no production Python wrapper."""
import json,os,sys,tempfile
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
from core.group import Group
from core.mesh import Mesh
class Viewport:
 def __init__(self,path):self.scene=Scene();self.history=History(self.scene);self.history.error_log=path;self.notifications=0
 def notify_scene_changed(self):self.notifications+=1
with tempfile.TemporaryDirectory(prefix="ingetrazo-ai-reference-")as tmp:
 v=Viewport(tmp+"/history.log");scope={"__name__":"__ai__"};out={}
 a=run_transactional(v,"mesh.add_edge(QVector3D(0,0,0),QVector3D(1,0,0))",scope);assert a["changed"] and len(v.history.undo_stack)==1
 v.history.undo();depth=len(v.history.redo_stack);b=run_transactional(v,"40+2",scope);assert not b["changed"] and len(v.history.redo_stack)==depth and scope["_"]==42
 out["mesh_and_inspect"]={"changed":a["changed"],"inspect":b,"redo_preserved":True}
 c=run_transactional(v,"mesh.add_edge(QVector3D(0,0,0),QVector3D(2,0,0))\ngroups.append(Group(Mesh(),name='added'))\nraise RuntimeError('controlled')",scope)
 assert not c["changed"] and not v.scene.groups and not v.scene.mesh.edges
 out["supported_rollback"]={"changed":c["changed"],"error_present":bool(c["error"]),"groups":len(v.scene.groups),"edges":len(v.scene.mesh.edges)}
 v.scene.groups.append(Group(Mesh(),name="existing"))
 d=run_transactional(v,"groups[0].name='changed-existing'\nscene.materials['Injected']={'color':(1,0,0)}\nraise RuntimeError('controlled')",scope)
 assert not d["changed"] and v.scene.groups[0].name=="changed-existing" and "Injected"in v.scene.materials
 out["partial_failure_defect"]={"changed":d["changed"],"error_present":bool(d["error"]),"existing_group_name":v.scene.groups[0].name,"material_survives":True}
 e=run_transactional(v,"groups[0].name='untracked-success'",scope)
 assert not e["changed"] and v.scene.groups[0].name=="untracked-success"
 out["unchanged_detection_defect"]={"changed":e["changed"],"existing_group_name":v.scene.groups[0].name}
 print(json.dumps(out,indent=2))
