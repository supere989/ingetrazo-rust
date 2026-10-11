#!/usr/bin/env python3
"""Retained source shape sessions versus actual native App controller states.
Coordinates use the existing drafting acceptance's five-decimal authoring boundary;
counts, oriented cycles, edge flags/curve groups, metadata and history stay strict.
No desktop or rendered-pixel claim.
"""
import argparse,copy,json,math
from collections import Counter
from pathlib import Path

def point(p):
 # Normalize signed zero only in comparison keys, never in source/native records.
 return tuple(0.0 if (q:=round(float(x),5))==0 else q for x in p)
def vector(p):
 assert type(p) is list and len(p)==3,'three-coordinate vector'
 assert all(type(x) in (int,float) and math.isfinite(x) for x in p),'finite numeric coordinate'
def indices(v,count,minimum):
 assert type(v) is list and len(v)>=minimum,'index inventory'
 assert all(type(i) is int and 0<=i<count for i in v),'integer in-range index'
def typed_state(s):
 assert type(s) is dict,'state object'
 for key in ('vertices','edges','faces','preview','history'):
  assert type(s[key]) is list,key+' array'
 for p in s['vertices']:vector(p)
 count=len(s['vertices'])
 for e in s['edges']:
  indices(e['vertices'],count,2);assert len(e['vertices'])==2,'edge pair'
  assert type(e['soft']) is bool and type(e['hidden']) is bool,'edge boolean flags'
  assert e['layer'] is None or type(e['layer']) is str,'edge layer'
  assert e['curve'] is None or (type(e['curve']) is int and e['curve']>=0),'curve identifier'
 for f in s['faces']:
  indices(f['vertices'],count,3);assert type(f['holes']) is list,'holes array'
  for h in f['holes']:indices(h,count,3)
  assert type(f['attrs']) is dict,'face attributes'
  assert type(f['interior']) is bool,'face interior boolean'
 for line in s['preview']:
  assert type(line) is list and len(line)==2,'preview pair'
  for p in line:vector(p)
 assert len(s['history'])==2 and all(type(x) is int and x>=0 for x in s['history']),'history integer counts'
 assert s['caption'] is None or type(s['caption']) is str,'caption type'
 if s.get('preview_color') is not None:
  c=s['preview_color'];assert type(c) is list and len(c) in (3,4),'preview color'
  assert all(type(x) in (int,float) and math.isfinite(x) for x in c),'numeric preview color'
def typed_phase(s):
 typed_state(s['state'])
 assert type(s['phase']) is str,'phase type'
 assert s.get('accepted') is None or type(s['accepted']) is bool,'accepted boolean'

def cycle(p):
 p=list(p);return min(tuple(p[i:]+p[:i]) for i in range(len(p))) if p else ()
def model(s):
 ps=[point(p) for p in s['vertices']];curves={}
 edges=[]
 for e in s['edges']:
  line=tuple(sorted(ps[i] for i in e['vertices']));edges.append((line,e['soft'],e['hidden'],e['layer']))
  if e['curve'] is not None:curves.setdefault(e['curve'],[]).append(line)
 faces=[]
 for f in s['faces']:
  faces.append((cycle([ps[i] for i in f['vertices']]),tuple(sorted(cycle([ps[i] for i in h]) for h in f['holes'])),json.dumps(f['attrs'],sort_keys=True),f['interior']))
 return (tuple(sorted(ps)),tuple(sorted(edges,key=repr)),tuple(sorted(faces,key=repr)),tuple(sorted(tuple(sorted(v)) for v in curves.values())))
def compare(source,native):
 assert len(source['cases'])==len(native['cases']), 'case inventory'
 rows=[];failures=[]
 for s,n in zip(source['cases'],native['cases']):
  assert s['recipe']==n['recipe'],'recipe identity'
  assert len(s['states'])==len(n['states']),'phase inventory'
  for ss,nn in zip(s['states'],n['states']):
   assert ss['phase']==nn['phase'],'phase identity'
   a,b=ss['state'],nn['state'];fields=[]
   try:typed_phase(ss);typed_phase(nn)
   except (AssertionError,KeyError,TypeError,ValueError) as e:
    row={'case':s['recipe']['name'],'phase':ss['phase'],'match':False,'differences':['typed state: '+str(e)]};rows.append(row);failures.append(row);continue
   if model(a)!=model(b):fields.append('geometry/topology/metadata')
   if a['history']!=b['history']:fields.append('history')
   if [list(map(point,line)) for line in a['preview']]!=[list(map(point,line)) for line in b['preview']]:fields.append('preview')
   if a['caption']!=b['caption']:fields.append('caption')
   if 'preview_color' in a and (None if a['preview_color'] is None else point(a['preview_color'][:3]))!=(None if b['preview_color'] is None else point(b['preview_color'][:3])):fields.append('preview_color')
   if ss.get('accepted') is not None and ss['accepted']!=nn.get('accepted'):fields.append('accepted')
   row={'case':s['recipe']['name'],'phase':ss['phase'],'match':not fields,'differences':fields};rows.append(row)
   if fields:failures.append(row)
 return rows,failures

def egui_ingress(native):
 """Bounded actual UI input recipe: origin click, near-X hover/click, Cancel.
 The (2,.03,0) cursor must axis-snap to (2,0,0); two-click tools commit,
 three-click tools retain a pending shape. Full gestures use source cases.
 """
 rows=[];failures=[]
 expected=['circle','polygon','arc','arc3','center_arc','pie','rotated_rect']
 data=native.get('egui_ingress')
 if type(data) is not list or [x.get('tool') for x in data if type(x) is dict]!=expected:
  return [],[{'tool':'inventory','differences':['seven ordered egui tool activations required']}]
 for r in data:
  tool=r['tool'];checks=[]
  try:
   states=[r[k] for k in ('cursor','pressed','hovered','second_pressed','cancel')]
   for st in states:typed_state(st)
   cur,pressed,hovered,second,cancel=states
   for st in states[:3]:
    assert not any(st[k] for k in ('vertices','edges','faces')) and st['history']==[0,0],'pending input mutated scene'
   assert all(st['last_error'] is None for st in states),'input error'
   checks.append('typed pending states without document mutation')
   if tool in ('circle','polygon'):
    count=24 if tool=='circle' else 6
    assert cur['caption']=='Sides' and len(cur['preview'])==count,'cursor ring'
    assert pressed['caption']=='Radius' and not pressed['preview'],'first click sets center'
    assert hovered['caption']=='Radius' and len(hovered['preview'])==count,'radial hover'
    assert point(hovered['preview'][0][0])==(2.0,0.0,0.0),'hover X-axis snap'
    assert [len(second[k]) for k in ('vertices','edges','faces')]==[count,count,1] and second['history']==[1,0],'second click commits one shape'
    assert all(abs(math.hypot(p[0],p[1])-2)<1e-5 and abs(p[2])<1e-5 for p in second['vertices']),'committed radius and plane'
    assert (2.0,0.0,0.0) in [point(p) for p in second['vertices']],'committed X-axis snap'
    assert second['caption']=='Sides' and cancel['caption']=='Sides','radial tool resets'
    assert model(second)==model(cancel) and second['history']==cancel['history'],'Cancel preserves completed geometry'
    checks.extend(['cursor ring and center click','hover and commit use X-axis snap','one authored face and history step','Cancel preserves completed geometry'])
   else:
    initial={'arc':'Bulge','arc3':None,'center_arc':'Radius','pie':'Radius','rotated_rect':'Length; angle'}[tool]
    assert cur['caption']==initial and pressed['caption']==initial and hovered['caption']==initial,'initial tool caption'
    assert not cur['preview'] and pressed['preview'] and hovered['preview']!=pressed['preview'],'click and motion reach pending tool'
    assert [point(p) for p in hovered['preview'][0]]==[(0.0,0.0,0.0),(2.0,0.0,0.0)],'hover X-axis snap'
    assert [point(p) for p in second['preview'][0]]==[(0.0,0.0,0.0),(2.0,0.0,0.0)],'second click baseline'
    next_caption={'center_arc':'Angle','pie':'Angle','rotated_rect':'Width; angle'}.get(tool,initial)
    assert second['caption']==next_caption,'second click advances tool stage'
    for st in (second,cancel):
     assert not any(st[k] for k in ('vertices','edges','faces')) and st['history']==[0,0],'pending/cancel scene preservation'
    assert not cancel['preview'] and cancel['caption']==initial,'Cancel clears pending tool'
    checks.extend(['click and hover reach tool','hover and second click use X-axis snap','second click retains pending geometry','Cancel clears pending geometry'])
   rows.append({'tool':tool,'match':True,'checks':checks})
  except (AssertionError,KeyError,TypeError,ValueError,IndexError) as e:
   row={'tool':tool,'match':False,'differences':[str(e)]};rows.append(row);failures.append(row)
 return rows,failures

def negative_controls(source,native,rows):
 controls=[]
 matching={(r['case'],r['phase']) for r in rows if r['match']}
 def candidate(predicate):
  return next(((i,j) for i,c in enumerate(native['cases']) for j,s in enumerate(c['states']) if (c['recipe']['name'],s['phase']) in matching and predicate(s)),None)
 changes=[
  ('lost-face',lambda s:bool(s['state']['faces']),lambda s:s['state']['faces'].pop()),
  ('wrong-metadata',lambda s:bool(s['state']['faces']),lambda s:s['state']['faces'][0]['attrs'].update(corruption=71)),
  ('lost-history',lambda s:s['state']['history'][0]>0,lambda s:s['state']['history'].__setitem__(0,999)),
  ('lost-curve',lambda s:any(e['curve'] is not None for e in s['state']['edges']),lambda s:[e.update(curve=None) for e in s['state']['edges']]),
  ('boolean-history',lambda s:True,lambda s:s['state']['history'].__setitem__(0,bool(s['state']['history'][0]))),
  ('integer-edge-flag',lambda s:bool(s['state']['edges']),lambda s:s['state']['edges'][0].update(soft=int(s['state']['edges'][0]['soft']))),
  ('integer-face-flag',lambda s:bool(s['state']['faces']),lambda s:s['state']['faces'][0].update(interior=int(s['state']['faces'][0]['interior']))),
  ('boolean-coordinate',lambda s:bool(s['state']['vertices']),lambda s:s['state']['vertices'][0].__setitem__(0,True)),
  ('integer-accepted',lambda s:type(s.get('accepted')) is bool,lambda s:s.update(accepted=int(s['accepted']))),
 ]
 for name,predicate,change in changes:
  target=candidate(predicate)
  if target is None:continue # E.g. a RotatedRectangle-only addendum has no curves.
  i,j=target;bad=copy.deepcopy(native);change(bad['cases'][i]['states'][j]);_,f=compare(source,bad)
  assert any(x['case']==source['cases'][i]['recipe']['name'] and x['phase']==source['cases'][i]['states'][j]['phase'] for x in f),name
  controls.append({'name':name,'rejected':True,'case':source['cases'][i]['recipe']['name'],'phase':source['cases'][i]['states'][j]['phase']})
 if not egui_ingress(native)[1]:
  bad=copy.deepcopy(native);bad.pop('egui_ingress');assert egui_ingress(bad)[1];controls.append({'name':'missing-egui-ingress','rejected':True})
  for i,r in enumerate(native['egui_ingress']):
   bad=copy.deepcopy(native);bad['egui_ingress'][i]['hovered']=copy.deepcopy(r['pressed'])
   assert any(x['tool']==r['tool'] for x in egui_ingress(bad)[1]),r['tool']
   controls.append({'name':'inert-egui-'+r['tool'],'rejected':True})
 return controls

def main():
 p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('native');p.add_argument('report');a=p.parse_args()
 source=json.loads(Path(a.source).read_text());native=json.loads(Path(a.native).read_text());rows,failures=compare(source,native)
 ui,ui_failures=egui_ingress(native)
 controls=negative_controls(source,native,rows)
 report={'cases':len(source['cases']),'phases':len(rows),'matching':sum(r['match'] for r in rows),'failures':failures,'egui_ingress':ui,'egui_failures':ui_failures,'negative_controls':controls,'rows':rows,'scope':'Model/controller and bounded actual egui input. Five-decimal geometric keys, oriented cycles and curve grouping; no raw vertex-ID identity or whole-frame renderer claim.'}
 Path(a.report).write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps({k:report[k] for k in ['cases','phases','matching','failures','egui_failures']}));raise SystemExit(bool(failures or ui_failures))
if __name__=='__main__':main()
