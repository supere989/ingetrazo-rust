"""Pinned operative public Vertex/registry commands and detached object history."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
cases=json.loads(Path(__file__).with_name('fixtures').joinpath('ai-detached-append.json').read_text());states=[]
for case in cases:
 s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
 def run(code):
  result=run_transactional(vp,code,scope);assert not result.get('error'),result
 run('m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nV=list(m.vertices)\nv=V[0]');m=scope['m'];V=scope['V']
 def observe(phase):
  def ident(v):return next((i for i,x in enumerate(V)if x is v),None)
  def xyz(v):return [v.position.x(),v.position.y(),v.position.z()]
  vertices=list(m.vertices)
  def add(v):
   if not any(x is v for x in vertices):vertices.append(v)
  for e in m.edges:add(e.v0);add(e.v1)
  for f in m.faces:
   for v in f.loop:add(v)
   for ring in f.hole_loops:
    for v in ring:add(v)
  for v in m._registry.values():add(v)
  edges=[[ident(e.v0),ident(e.v1)]for e in m.edges]
  faces=[{'outer':[ident(v)for v in f.loop],'holes':[[ident(v)for v in ring]for ring in f.hole_loops]}for f in m.faces]
  states.append({'name':case['name']+'/'+phase,'public':[ident(v)for v in m.vertices],'positions':[xyz(v)for v in V],'edges':edges,'faces':faces,'registry':[{'key':[str(cell)for cell in key],'vertex':ident(v)}for key,v in m._registry.items()],'native_edges':edges,'native_faces':faces,'native_positions':[{'identity':ident(v),'position':xyz(v)}for v in vertices],'undo':len(h.undo_stack),'redo':len(h.redo_stack)})
 run(case['before']);run(case['append']);observe('appended');assert h.undo();observe('undone');assert h.redo();observe('redone');assert h.undo();observe('undone-again');assert h.redo();observe('redone-again')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
