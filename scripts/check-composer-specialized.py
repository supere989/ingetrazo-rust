#!/usr/bin/env python3
"""Strict actual specialized composer state/readers and independently owned opaque guards."""
import argparse,copy,json,math,zipfile
from pathlib import Path
NAMES=['linear-handles','angular-radius','radial-endpoint','label-leaders','label-wrap-width','callout-bubble-and-box','locked-hidden-handle-admission','chain-fresh-jog','chain-anchored-undo','chain-seeded-and-switch','baseline-fresh-and-seeded','multi-ctrl-properties','multi-box-admission','multi-group-move-delete']
FLAGS=['idle_reused','atomic_refusal','actual_pointer_ingress']
def equal(a,b,path='$'):
 if type(a)in(int,float)and type(b)in(int,float):
  assert math.isfinite(a)and math.isfinite(b),path+': nonfinite'
  assert a==b and (a!=0 or math.copysign(1,a)==math.copysign(1,b)),(path,a,b)
  return
 assert type(a)is type(b),(path,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys',sorted(a),sorted(b))
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+f'[{i}]')
 else:assert a==b,(path,a,b)
def load_igz(p):
 b=p.read_bytes();return json.loads(zipfile.ZipFile(p).read('document.json'))if b[:2]==b'PK'else json.loads(b)
def at(doc,path):
 for k in path.split('.'):doc=doc[int(k)]if isinstance(doc,list)else doc[k]
 return doc
def verify(s,n,sr,nr,native):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in s['cases']]==NAMES and [c['name']for c in n['cases']]==NAMES
 expected=[]
 for c in s['cases']:expected.extend((c['name'],a['phase'])for a in c['actions']);expected.append((c['name'],'save'))
 assert len(expected)==220
 assert [(r['case'],r['phase'])for r in s['states']]==expected and [(r['case'],r['phase'])for r in n['states']]==expected
 for flag in FLAGS:assert n['controls'][flag]is True,flag
 for i,(a,b)in enumerate(zip(s['states'],n['states'])):
  for key in ['undo','redo','sheet_index']:
   assert type(a[key])is int and type(b[key])is int and a[key]>=0 and b[key]>=0,(i,key)
  equal(a,b,f'states[{i}]')
 assert [r['case']for r in sr]==NAMES and [r['case']for r in nr]==NAMES
 equal(sr,nr,'original_reader_pairs')
 for guard in s['opaque_guards']:
  case=next(c for c in n['cases']if c['name']==guard['case']);p=(native/case['saved']).resolve();p.relative_to(native.resolve());doc=load_igz(p)
  for path,value in guard['saved_paths'].items():equal(value,at(doc,path),'opaque.'+path)
def main():
 p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args();s=json.loads((a.source/'original-specialized.json').read_text());n=json.loads((a.native/'native-specialized.json').read_text());sr=json.loads((a.source/'original-reopens.json').read_text());nr=json.loads((a.native/'native-reopens.json').read_text());report=dict(ok=False,state_mismatches=[])
 for i,(x,y)in enumerate(zip(s['states'],n['states'])):
  try:equal(x,y,f'states[{i}]')
  except AssertionError as e:report['state_mismatches'].append(dict(case=x['case'],phase=x['phase'],difference=str(e)))
 try:
  verify(s,n,sr,nr,a.native);mutations=[('extra-state',lambda d:d['states'].append(copy.deepcopy(d['states'][-1]))),('missing-state',lambda d:d['states'].pop()),('phase-order',lambda d:d['states'].reverse()),('selection-membership',lambda d:d['states'][1]['selection'].clear()),('history',lambda d:d['states'][0].__setitem__('undo',1)),('history-bool',lambda d:d['states'][0].__setitem__('undo',False)),('composition',lambda d:d['states'][0]['composition']['cotas'][0].__setitem__('dx_mm',999.)),('run-axis',lambda d:d['states'][0]['pending'].__setitem__('run_axis','v')),('run-point',lambda d:d['states'][0]['pending']['run_points'].append([1.,2.])),('sheet-owner',lambda d:d['states'][0].__setitem__('sheet_index',1))]
  for flag in FLAGS:mutations.append((flag,lambda d,k=flag:d['controls'].__setitem__(k,False)))
  rejected=[]
  for label,mutate in mutations:
   bad=copy.deepcopy(n);mutate(bad)
   try:verify(s,bad,sr,nr,a.native)
   except AssertionError:rejected.append(label)
   else:raise AssertionError('Negative did not reject: '+label)
  badsource=copy.deepcopy(s);badsource['opaque_guards'][0]['saved_paths']['scene.compositions.0.vendor_specialized']=None
  try:verify(badsource,n,sr,nr,a.native)
  except AssertionError:rejected.append('opaque-raw-value')
  else:raise AssertionError('Opaque negative did not reject')
  bad=copy.deepcopy(nr);bad.pop()
  try:verify(s,n,sr,bad,a.native)
  except AssertionError:rejected.append('missing-reader')
  else:raise AssertionError('Reader negative did not reject')
  report.update(ok=True,cases=14,states=220,original_reader_pairs=14,rejected_controls=rejected,source_radial_population_errors=len(s['callback_errors']),scope='Literal source geometry/history/run/annotation-multi states. Radial original UI population exceptions retained diagnostically; no source success substituted. No physical pixel/general-sheet multi claim.')
 except Exception as e:
  report['error']=repr(e);a.comparison.parent.mkdir(parents=True,exist_ok=True);a.comparison.write_text(json.dumps(report,indent=2)+'\n');raise
 a.comparison.parent.mkdir(parents=True,exist_ok=True);a.comparison.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
