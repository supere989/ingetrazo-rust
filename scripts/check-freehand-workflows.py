#!/usr/bin/env python3
"""Paired Freehand handlers and bounded real native egui ingress; no pixel claim."""
import argparse,copy,json,importlib.util
from pathlib import Path
spec=importlib.util.spec_from_file_location('shape_checker',Path(__file__).with_name('check-shape-drafting.py'));h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
ROSTER=json.loads((Path(__file__).parent.parent/'docs/audits/freehand-workflows/roster.json').read_text())
def line(a,b):return tuple(sorted((h.point(a),h.point(b))))
def ingress(n):
 rows=n['egui_ingress'];assert [r['mode']for r in rows]==['closed-release','open-release','cancel-repeat'],'ordered egui roster'
 for r in rows:
  for k in ['initial','press','hover','before_release','release','repeated','undo','redo']:h.typed_state(r[k])
  points=r['expected_world'];assert len(points)==5
  for p in points+r['nearby_world']:h.vector(p)
  assert len(r['initial']['edges'])==1 and r['initial']['history']==[0,0]
  assert h.model(r['press'])==h.model(r['initial']) and r['press']['history']==[0,0]
  count=3 if r['mode']=='open-release' else 5
  expected_preview=[[h.point(a),h.point(b)]for a,b in zip(points[:count-1],points[1:count])]
  assert [[h.point(a),h.point(b)]for a,b in r['hover']['preview']]==expected_preview,'raw authored hover positions'
  assert h.model(r['hover'])==h.model(r['initial']),'immutable model during drag'
  if r['mode']=='cancel-repeat':
   assert r['before_release']['preview']==[] and h.model(r['release'])==h.model(r['initial']) and r['release']['history']==[0,0]
  else:assert r['release']['history']==[1,0] and not r['release']['preview']
  committed=r['repeated'];assert committed['history']==[1,0]
  expected_edges=[line(*r['nearby_world'])]+[line(a,b)for a,b in zip(points[:count-1],points[1:count])]
  actual_edges=[line(*(committed['vertices'][i]for i in e['vertices']))for e in committed['edges']]
  assert sorted(actual_edges)==sorted(expected_edges),'raw pixel-to-plane edges, no vertex/axis snap'
  expected_vertices=r['nearby_world']+points[:count if count==3 else 4]
  assert sorted(map(h.point,committed['vertices']))==sorted(map(h.point,expected_vertices)),'raw pixel-to-plane vertices'
  assert len(committed['faces'])==(0 if count==3 else 1)
  assert sum(e['curve']is not None for e in committed['edges'])==count-1
  assert h.model(r['undo'])==h.model(r['initial']) and r['undo']['history']==[0,1]
  assert r['redo']['history']==[1,0] and h.model(r['redo'])==h.model(committed)
 return len(rows)
def compare(s,n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9','original pin'
 assert [{'name':c['recipe']['name'],'phases':[x['phase']for x in c['states']]}for c in s['cases']]==ROSTER,'frozen source recipe/phase roster'
 rows,failures=h.compare(s,n)
 for c in s['cases']+n['cases']:
  for phase in c['states']:
   if phase['state'].get('last_error') is not None:failures.append({'case':c['recipe']['name'],'phase':phase['phase'],'match':False,'differences':['unexpected operation error']})
 return rows,failures
def main():
 p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('native');p.add_argument('report');a=p.parse_args();s=json.loads(Path(a.source).read_text());n=json.loads(Path(a.native).read_text());rows,failures=compare(s,n);egui_error=None
 try:count=ingress(n)
 except(AssertionError,KeyError,TypeError)as e:count=0;egui_error=str(e)
 controls=[]
 # Negative controls run only against the accepted positive baseline; unrelated
 # preexisting failures can never authenticate a mutation.
 if not failures and egui_error is None:
  def reject(name,mutate):
   wrong=copy.deepcopy(n);mutate(wrong)
   try:r,f=compare(s,wrong);ingress(wrong);detected=bool(f)
   except(AssertionError,KeyError,TypeError,ValueError):detected=True
   assert detected,name;controls.append(dict(name=name,rejected=True))
  reject('missing-case',lambda x:x['cases'].pop())
  reject('wrong-typed-history',lambda x:x['cases'][0]['states'][0]['state']['history'].__setitem__(0,False))
  idx=next(i for i,c in enumerate(n['cases'])if c['recipe']['name']=='vertical-plane');phase=next(i for i,t in enumerate(n['cases'][idx]['states'])if t['state']['faces'])
  reject('lost-seam-face',lambda x:x['cases'][idx]['states'][phase]['state'].__setitem__('faces',[]))
  reject('lost-curve',lambda x:[e.update(curve=None)for e in x['cases'][idx]['states'][phase]['state']['edges']])
  preview=next(i for i,t in enumerate(n['cases'][0]['states'])if t['state']['preview'])
  reject('lost-sampling-preview',lambda x:x['cases'][0]['states'][preview]['state'].__setitem__('preview',[]))
  reject('inert-egui',lambda x:x.__setitem__('egui_ingress',[]))
  reject('cancel-committed',lambda x:x['egui_ingress'][2].__setitem__('release',copy.deepcopy(x['egui_ingress'][2]['repeated'])))
  reject('snapped-egui-vertex',lambda x:x['egui_ingress'][0]['repeated']['vertices'].__setitem__(2,x['egui_ingress'][0]['nearby_world'][0]))
  for label,edit in [('jointly-empty',lambda x:x.__setitem__('cases',[])),('jointly-truncated',lambda x:x['cases'].pop())]:
   ss,nn=copy.deepcopy(s),copy.deepcopy(n);edit(ss);edit(nn)
   try:compare(ss,nn);detected=False
   except(AssertionError,KeyError,TypeError):detected=True
   assert detected;controls.append(dict(name=label,rejected=True))
 report=dict(cases=len(s['cases']),phases=len(rows),matching=sum(r['match']for r in rows),failures=failures,egui_sessions=count,egui_error=egui_error,negative_controls=controls,scope='Pinned original Freehand handlers versus actual native App dispatch;3nativeegui raw pixel-to-plane press/held-motion/release/cancel/repeat/Undo/Redo and nearby unsnapped geometry; accepted5decimal model boundary, no rendered/desktop/rawUUID claim.')
 Path(a.report).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));assert not failures and egui_error is None
if __name__=='__main__':main()
