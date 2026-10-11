"""Pinned operative public registry object/content history control."""
import json,sys,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code):
 r=run_transactional(vp,code,scope);assert not r.get('error'),r
run("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nr=m._registry\nv=m.vertices[0]\nassert r is m._registry\nassert r[(0,0,0)] is v\nassert m.vertex_at(QVector3D()) is v")
run("r[(90000,0,0)]=v\nassert m._lookup(QVector3D(9,0,0)) is v")
assert h.undo();run("assert r is m._registry\nassert (90000,0,0) not in r\nassert r[(0,0,0)] is v")
assert h.redo();run("assert r is m._registry\nassert r[(90000,0,0)] is v")
r=run_transactional(vp,"r.clear()\nraise ValueError('restore')",scope);assert r.get('error')
run("assert r is m._registry\nassert r[(0,0,0)] is v\nassert r[(90000,0,0)] is v")
print(json.dumps({'persistent_dictionary':True,'undo_redo_owned_contents':True,'failed_command_restore':True,'production_enabled':False}))
