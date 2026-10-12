#!/usr/bin/env python3
"""Strict decoded figure ownership and unchanged default-export semantics."""
import copy,importlib.util,json,struct,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('rich',Path(__file__).with_name('check-gltf-rich.py'));rich=importlib.util.module_from_spec(spec);spec.loader.exec_module(rich)
def document(path):
 data=Path(path).read_bytes();size,kind=struct.unpack_from('<II',data,12);assert kind==0x4e4f534a
 return json.loads(data[20:20+size])
def decoded(path):
 d=rich.decode(path);doc=document(path)
 d['mesh_inventory']=[dict(name=m.get('name'),materials=[p['material'] for p in m['primitives']],count=len(m['primitives'])) for m in doc.get('meshes',[])]
 assert len(d['nodes'])==len(d['mesh_inventory'])
 assert [n['mesh'] for n in d['nodes']]==list(range(len(d['mesh_inventory'])))
 assert d['scenes'][0]['nodes']==list(range(len(d['nodes'])))
 return d
def main():
 source=Path(sys.argv[1]);native=Path(sys.argv[2]);cases=json.loads((source/'cases.json').read_text());assert len(cases)==8 and len({c['name'] for c in cases})==8
 results={};rejects=0
 for c in cases:
  name=c['name'];a=decoded(source/(name+'.glb'));b=decoded(native/(name+'.glb'));rich.validate(a,b)
  rich.validate(decoded(source/(name+'.ordinary.glb')),decoded(native/(name+'.ordinary.glb')))
  for n in b['nodes']:
   if n['name'].startswith('Figure '):assert len(n['extras']['ingetrazo_faceme'])==4
  results[name]=dict(original=a,native=b)
  mutations=[]
  if b['nodes']:
   x=copy.deepcopy(b);x['nodes'][0]['mesh']=999;mutations.append(x)
  figures=[i for i,n in enumerate(b['nodes']) if n['name'].startswith('Figure ')]
  if figures:
   x=copy.deepcopy(b);x['nodes'][figures[0]]['extras']['ingetrazo_faceme'][0]+=1;mutations.append(x)
   x=copy.deepcopy(b);del x['nodes'][figures[0]]['extras'];mutations.append(x)
  if b['materials']:
   x=copy.deepcopy(b);x['materials'][0]['name']='wrong-owner';mutations.append(x)
  for x in mutations:
   try:rich.validate(a,x)
   except AssertionError:rejects+=1
   else:raise AssertionError('negative accepted')
 if len(sys.argv)>3 and not sys.argv[3].startswith('--'):Path(sys.argv[3]).write_text(json.dumps(results,indent=2)+'\n')
 initial=json.loads((native/'initial-decoded-failure.json').read_text());failed=[]
 assert set(initial)=={c['name'] for c in cases}
 for name,row in initial.items():
  try:rich.validate(row['original'],row['native'])
  except AssertionError:failed.append(name)
 assert set(failed)=={'mesh-perspective','bitmap-perspective','mixed-first-material','identity-orphan'}
 path_only=json.loads((native/'path-only-candidate-failure.json').read_text());asset_failures=[]
 assert set(path_only)==set(initial)
 for name,row in path_only.items():
  try:rich.validate(row['original'],row['native'])
  except AssertionError:asset_failures.append(name)
 assert set(asset_failures)=={'bitmap-perspective','figure-only','mixed-first-material'}
 assert decoded(source/'collapse.glb')['nodes']==[]
 refusal=json.loads((native/'collapse-refusal.json').read_text());assert refusal['refused'] and refusal['writer_unchanged'] and refusal['scene_unchanged']
 worker_frames=0
 if '--worker' in sys.argv:
  from PySide6.QtGui import QImage
  app=json.loads((native/'application'/'result.json').read_text())
  assert [x['engine'] for x in app['engines']]==['cycles','eevee']
  assert all(x['same_process_and_model'] and x['restore_exact'] and x['save_exact'] for x in app['engines'])
  assert all(app[k] for k in ['cancel_reaped','spawn_failure_preserves_image','document_and_history_unchanged','follow_rest_mouse_hold_latest_camera','follow_config_reload','follow_three_file_retention','follow_stop_reaped'])
  for engine in ['cycles','eevee']:
   rich.validate(decoded(source/'worker'/engine/'model.glb'),decoded(native/'application'/(engine+'-model.glb')))
   for phase,nativephase in [('first','first'),('camera','camera'),('restored','saved')]:
    a=QImage(str(source/'worker'/engine/(phase+'.png'))).convertToFormat(QImage.Format_RGBA8888)
    b=QImage(str(native/'application'/(engine+'-'+nativephase+'.png'))).convertToFormat(QImage.Format_RGBA8888)
    assert not a.isNull() and not b.isNull() and a.size()==b.size()
    assert bytes(a.constBits())==bytes(b.constBits()),(engine,phase,'worker frame differs')
    worker_frames+=1
 print(json.dumps(dict(cases=8,ordinary=8,negative_controls=rejects,retained_failed_candidates=len(failed)+len(asset_failures),unsupported_collapse_atomic_refusal=1,worker_exact_frames=worker_frames)))
if __name__=='__main__':main()
