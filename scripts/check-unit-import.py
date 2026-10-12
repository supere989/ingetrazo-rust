#!/usr/bin/env python3
"""Strict missing actual OBJ/DXF ingress and face cleanup workflow checker."""
import copy,importlib.util,json,math,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('exchange_strict',Path(__file__).with_name('check-exchange-import.py'));strict=importlib.util.module_from_spec(spec);spec.loader.exec_module(strict);equal=strict.equal
NAMES=['obj-root','obj-texture-leaf','dxf-block-leaf','dxf-3dface','dxf-solid','dxf-trace','dxf-face-block','obj-unit-cancel','dxf-unit-cancel','obj-chooser-cancel','dxf-chooser-cancel','obj-invalid','dxf-invalid']
GUARDS=['actual_chooser','actual_unit_buttons','cancel_failure_preserved','idle_readonly','camera_policy','preferences_reopened']
def compare(s,n,r):
 assert s['source_pin']==n['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9' and s['callback_errors']==[]
 assert [c['name']for c in s['cases']]==NAMES
 equal([{k:c[k]for k in ['name','saved','phases']}for c in s['cases']],n['cases'],'cases')
 assert len(s['states'])==len(n['states'])==47
 def overlay(states):
  rows=[]
  for state in states:
   v=state['camera']['distance'];assert type(v)in(int,float)and math.isfinite(v)and v>0
   number=float(v)
   if type(v)is int:assert int(number)==v
   rows.append(dict(state,camera=dict(state['camera'],distance=number)))
  return rows
 assert set(n['controls'])==set(GUARDS)
 for k in GUARDS:assert n['controls'][k]is True,k
 equal(overlay(s['states']),overlay(n['states']),'states')
 names=[c['name']for c in s['cases']if c['saved']is not None];assert len(names)==7 and names==[v['case']for v in r]
 for row in r:equal(row['original'],row['native'],'reader.'+row['case'])
 return {'cases':13,'states':47,'original_readers':7,'operative_guards':6}
def main():
 source,native,output=map(Path,sys.argv[1:]);s=json.loads((source/'original-unit-import.json').read_text());n=json.loads((native/'native-unit-import.json').read_text());r=json.loads((native/'reopens-unit-import.json').read_text());result={'passed':False}
 try:
  result.update(compare(s,n,r));rejected=[]
  # A single sparse candidate at a time; shared untouched payloads are read-only.
  def fork(v,path):
   if not path:return copy.copy(v)
   out=copy.copy(v);out[path[0]]=fork(v[path[0]],path[1:]);return out
  def mutations():
   for k in GUARDS:
    v=fork(n,['controls']);v['controls'][k]=False;yield k,v,r
   for name,path,value in [('coordinate',['states',1,'scene','active','points',0,0],999),('camera-target',['states',1,'camera','target',0],999),('camera-yaw',['states',1,'camera','yaw'],999),('camera-pitch',['states',1,'camera','pitch'],999),('camera-distance',['states',1,'camera','distance'],999),('camera-bool',['states',1,'camera','distance'],True),('camera-inf',['states',1,'camera','distance'],float('inf')),('history',['states',1,'undo'],999),('owner',['states',1,'depth'],999),('metadata',['states',1,'scene','units'],'bad')]:
    v=fork(n,path[:-1]);parent=v
    for k in path[:-1]:parent=parent[k]
    parent[path[-1]]=value;yield name,v,r
   v=dict(n,states=n['states'][:-1]);yield 'inventory',v,r
   q=list(r);q[0]=dict(q[0],native={});yield 'reader',n,q
  for name,v,q in mutations():
   try:compare(s,v,q)
   except (AssertionError,ValueError,TypeError,KeyError,IndexError):rejected.append(name)
   else:raise AssertionError('Accepted mutation '+name)
  result.update(passed=True,negative_controls=len(rejected),rejected=rejected)
 except Exception as e:result['error']=repr(e)
 output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));return 0 if result['passed']else 1
if __name__=='__main__':raise SystemExit(main())
