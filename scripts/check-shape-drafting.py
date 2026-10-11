#!/usr/bin/env python3
"""Retained source shape sessions versus actual native App controller states.
Coordinates use the existing drafting acceptance's five-decimal authoring boundary;
counts, oriented cycles, edge flags/curve groups, metadata and history stay strict.
No desktop or rendered-pixel claim.
"""
import argparse,copy,json
from collections import Counter
from pathlib import Path

def point(p):return tuple(round(float(x),5) for x in p)
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
   if model(a)!=model(b):fields.append('geometry/topology/metadata')
   if a['history']!=b['history']:fields.append('history')
   if [list(map(point,line)) for line in a['preview']]!=[list(map(point,line)) for line in b['preview']]:fields.append('preview')
   if a['caption']!=b['caption']:fields.append('caption')
   if 'preview_color' in a and (None if a['preview_color'] is None else point(a['preview_color'][:3]))!=(None if b['preview_color'] is None else point(b['preview_color'][:3])):fields.append('preview_color')
   if ss.get('accepted') is not None and ss['accepted']!=nn.get('accepted'):fields.append('accepted')
   row={'case':s['recipe']['name'],'phase':ss['phase'],'match':not fields,'differences':fields};rows.append(row)
   if fields:failures.append(row)
 return rows,failures

def main():
 p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('native');p.add_argument('report');a=p.parse_args();source=json.loads(Path(a.source).read_text());native=json.loads(Path(a.native).read_text());rows,failures=compare(source,native)
 controls=[]
 # Invoke the same validator, mutating only an existing positive authored phase.
 target=next(( (i,j) for i,c in enumerate(native['cases']) for j,s in enumerate(c['states']) if s['state']['faces']),None)
 if target:
  i,j=target
  for name,change in [('lost-face',lambda s:s['faces'].pop()),('wrong-metadata',lambda s:s['faces'][0]['attrs'].update({'corruption':71})),('lost-history',lambda s:s['history'].__setitem__(0,999)),('lost-curve',lambda s:[e.update(curve=None) for e in s['edges']])]:
   bad=copy.deepcopy(native);change(bad['cases'][i]['states'][j]['state']);_,f=compare(source,bad);assert any(x['case']==source['cases'][i]['recipe']['name'] and x['phase']==source['cases'][i]['states'][j]['phase'] for x in f);controls.append({'name':name,'rejected':True})
 report={'cases':len(source['cases']),'phases':len(rows),'matching':sum(r['match'] for r in rows),'failures':failures,'negative_controls':controls,'rows':rows,'scope':'Model/controller/egui input; no whole-frame renderer claim'};Path(a.report).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k] for k in ['cases','phases','matching','failures']}));raise SystemExit(bool(failures))
if __name__=='__main__':main()
