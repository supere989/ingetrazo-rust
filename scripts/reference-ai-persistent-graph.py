"""Frozen source topology identity/history baseline; generated membership and field history control."""
import json,sys,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code):
 r=run_transactional(vp,code,scope);assert not r.get('error'),r
run("m=mesh\nf0=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nf1=m.add_face([QVector3D(1,0,0),QVector3D(0,0,0),QVector3D(0,0,1)])\nv=m.vertices[0]\ne=m.edges[0]\noldvedges=v.edges\noldefaces=e.faces\noldvertices=m.vertices\noldedges=m.edges\noldfaces=m.faces\ntag=['opaque',7]")
m=scope['m'];v=scope['v'];e=scope['e']
vertices=list(m.vertices);edges=list(m.edges);faces=list(m.faces)
def ident(x,items):return next((i for i,item in enumerate(items) if x is item),None)
states=[]
def record(name):
 states.append(dict(name=name,vertices=[ident(x,vertices) for x in m.vertices],edges=[ident(x,edges)for x in m.edges],faces=[ident(x,faces)for x in m.faces],incidence=[sorted(ident(x,edges)for x in item.edges)for item in vertices],radial=[[ident(x,faces)for x in item.faces]for item in edges],endpoints=[[ident(item.v0,vertices),ident(item.v1,vertices)]for item in edges],persistent_lists=[m.vertices is scope['oldvertices'],m.edges is scope['oldedges'],m.faces is scope['oldfaces']],held_incidence_original=v.edges is scope['oldvedges'],held_radial_original=e.faces is scope['oldefaces'],soft_tag_identity=e.soft is scope['tag'],layer_tag_identity=e.layer is scope['tag'],curve_tag_identity=e.curve is scope['tag'],undo=len(h.undo_stack),redo=len(h.redo_stack)))
record('initial')
run("e.faces.reverse()\nv.edges.remove(e)\ne.soft=tag\ne.layer=tag\ne.curve=tag");record('fields-edited')
assert h.undo();record('fields-undone')
assert h.redo();record('fields-redone')
r=run_transactional(vp,"e.faces.clear()\nv.edges.clear()\ne.layer=None\nraise ValueError('reject')",scope);assert r.get('error'),r
record('error-restored')
assert h.undo();record('repeat-undone')
assert h.redo();record('repeat-redone')
run("f2=m.add_face([QVector3D(0,1,0),QVector3D(1,0,0),QVector3D(1,1,1)])")
vertices=list(m.vertices);edges=list(m.edges);faces=list(m.faces)
record('append-shared')
assert h.undo();record('append-undone-detached-held')
assert h.redo();record('append-redone')
r=run_transactional(vp,"e.v0=m.vertices[2]\nraise ValueError('endpoint-retained')",scope);assert r.get('error'),r
record('endpoint-error-retained')
assert h.undo();record('endpoint-undo-retained')
assert h.redo();record('endpoint-redo-retained')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
