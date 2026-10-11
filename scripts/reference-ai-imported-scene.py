"""Operative current Scene mesh -> run_python command -> Undo/Redo."""
import json,types,sys
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
from PySide6.QtGui import QVector3D
s=Scene();f=s.mesh.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)]);f.attrs.update({'color':[0.2,0.4,0.8],'layer':'Imported'});h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);states=[]
def observe(name):states.append(json.loads(json.dumps({'name':name,'faces':[{'attrs':f.attrs,'layer':f.attrs.get('layer')}for f in s.mesh.faces],'undo':len(h.undo_stack),'redo':len(h.redo_stack)})))
observe('imported');result=run_transactional(vp,"m=mesh\nf=m.faces[0]\nf.attrs['mat']='Edited'\nf.loop[0].position.setZ(2)",scope);assert not result.get('error'),result;observe('edited');assert s.mesh.vertices[0].position.z()==2;assert h.undo();observe('undone');assert s.mesh.vertices[0].position.z()==0;assert h.redo();observe('redone')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
