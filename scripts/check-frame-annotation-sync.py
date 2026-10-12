#!/usr/bin/env python3
"""Strict source/native frame anchor and overlay semantic acceptance; no pixel claim."""
import argparse,copy,json,math,struct
from pathlib import Path
NAMES=['anchored-frame-transforms','anchored-model-history','anchor-tolerance-and-orphans','label-leaders-and-level','model-dimensions-and-text','paths-and-chainage','section-traces','saved-view-and-perspective']
FLAGS=['idle_reused','changed_invalidated','actual_cota_ingress','actual_frame_move']
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
def verify(s,n,sr,nr):
 assert [c['name']for c in s['cases']]==NAMES
 assert [c['name']for c in n['cases']]==NAMES
 expected=[(c['name'],a['phase'])for c in s['cases']for a in c['actions']]
 assert len(expected)==58
 assert [(r['case'],r['phase'])for r in s['states']]==expected
 assert [(r['case'],r['phase'])for r in n['states']]==expected
 assert s['source_errors']==[]
 for flag in FLAGS:assert n['controls'][flag]is True,flag
 for i,(a,b)in enumerate(zip(s['states'],n['states'])):
  assert a['viewport_restored']is True and b['viewport_restored']is True
  assert type(a['source_errors'])is int and a['source_errors']==0
  assert type(b['source_errors'])is int and b['source_errors']==0
  for key in ['sheet_undo','sheet_redo','model_undo','model_redo']:
   assert type(a[key])is int and type(b[key])is int and a[key]>=0 and b[key]>=0,(i,key)
  equal(a,b,f'states[{i}]')
 assert [r['case']for r in sr]==NAMES and [r['case']for r in nr]==NAMES
 equal(sr,nr,'original_reader_pairs')
def diagnostics(s,n,sr,nr):
 rows=[]
 for i,(a,b)in enumerate(zip(s.get('states',[]),n.get('states',[]))):
  try:equal(a,b,f'states[{i}]')
  except AssertionError as e:rows.append({'case':a.get('case'),'phase':a.get('phase'),'first_difference':str(e)})
 readers=[]
 for a,b in zip(sr,nr):
  try:equal(a,b,'reader')
  except AssertionError as e:readers.append({'case':a.get('case'),'first_difference':str(e)})
 return dict(state_mismatches=rows,reader_mismatches=readers)
def main():
 p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);args=p.parse_args()
 s=json.loads((args.source/'original-frame-annotations.json').read_text());n=json.loads((args.native/'native-frame-annotations.json').read_text());sr=json.loads((args.source/'original-reopens.json').read_text());nr=json.loads((args.native/'native-reopens.json').read_text());report=dict(ok=False,**diagnostics(s,n,sr,nr))
 try:
  verify(s,n,sr,nr)
  mutations=[]
  mutations.append(('extra-state',lambda d:d['states'].append(copy.deepcopy(d['states'][-1]))))
  mutations.append(('missing-state',lambda d:d['states'].pop()))
  mutations.append(('reordered-state',lambda d:d['states'].reverse()))
  for flag in FLAGS:mutations.append((flag,lambda d,f=flag:d['controls'].__setitem__(f,False)))
  for key in ['sheet_undo','sheet_redo','model_undo','model_redo']:
   mutations.append((key,lambda d,k=key:d['states'][0].__setitem__(k,d['states'][0][k]+1)))
   mutations.append((key+'-bool',lambda d,k=key:d['states'][0].__setitem__(k,False)))
  mutations.append(('viewport-restoration',lambda d:d['states'][0].__setitem__('viewport_restored',False)))
  mutations.append(('callback-error',lambda d:d['states'][0].__setitem__('source_errors',1)))
  mutations.append(('overlay-order',lambda d:d['states'][0]['overlay_by_frame']['main'].reverse()))
  mutations.append(('geometry',lambda d:d['states'][0]['model_geometry']['vertices'][0].__setitem__(0,.125)))
  mutations.append(('missing-frame-record',lambda d:d['states'][0]['composition']['frames'].pop()))
  rejected=[]
  for label,mutate in mutations:
   bad=copy.deepcopy(n);mutate(bad)
   try:verify(s,bad,sr,nr)
   except AssertionError:rejected.append(label)
   else:raise AssertionError('Negative did not reject: '+label)
  bad=copy.deepcopy(nr);bad.pop()
  try:verify(s,n,sr,bad)
  except AssertionError:rejected.append('missing-reader')
  else:raise AssertionError('Reader negative did not reject')
  report.update(ok=True,cases=8,states=58,original_reader_pairs=8,rejected_controls=rejected,scope='Strict semantic tuples/records and history. Numeric JSON int/float spelling is equivalent; booleans/types/keys/order and signed zero remain strict. Physical pixels and fast20k resource branch are separate.')
 except Exception as error:
  report['error']=repr(error);args.comparison.parent.mkdir(parents=True,exist_ok=True);args.comparison.write_text(json.dumps(report,indent=2)+'\n');raise
 args.comparison.parent.mkdir(parents=True,exist_ok=True);args.comparison.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
