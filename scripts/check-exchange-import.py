#!/usr/bin/env python3
"""Strict finite App comparison; exact camera distance and one declared native correction."""
import sys,json,math,copy
from pathlib import Path
NAMES=['dae-z-root','dae-y-inch-leaf','dae-400','dae-401-leaf','dae-datum','dae-existing-datum','dae-combined-leaf','dae-mixed-textures','glb-named','glb-textured-leaf','gltf-scale','chooser-cancel','dae-failure-datum']
GUARDS=['actual_chooser','worker_file_only','parse_once','camera_unchanged','atomic_failure','idle_readonly','context_retired']
def equal(a,b,path=''):
 if type(a)is not type(b):raise AssertionError(f'{path}: type {type(a).__name__}/{type(b).__name__}')
 if isinstance(a,dict):
  if a.keys()!=b.keys():raise AssertionError(f'{path}: keys differ')
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  if len(a)!=len(b):raise AssertionError(f'{path}: length {len(a)}/{len(b)}')
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,f'{path}[{i}]')
 elif isinstance(a,float):
  if not(math.isfinite(a)and math.isfinite(b))or a!=b or(a==0 and math.copysign(1,a)!=math.copysign(1,b)):raise AssertionError(f'{path}: {a!r}/{b!r}')
 elif a!=b:raise AssertionError(f'{path}: {a!r}/{b!r}')
def compare(source,native,readers):
 assert source['source_pin']==native['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert source['callback_errors']==[]
 assert [c['name']for c in source['cases']]==NAMES
 equal([{k:c[k]for k in['name','saved','phases']}for c in source['cases']],native['cases'],'cases')
 a=copy.deepcopy(source['states']);b=copy.deepcopy(native['states']);assert len(a)==61 and len(b)==61
 before=next(s for s in a if s['case']=='dae-failure-datum'and s['phase']=='before');failed=next(s for s in a if s['case']=='dae-failure-datum'and s['phase']=='import');assert before['scene']['datum']is None and failed['scene']['datum']is not None
 # Preserve source evidence unchanged. This one native expectation is a named
 # atomicity correction, not accepted literal source failure behavior.
 failed['scene']['datum']=None
 for rows in[a,b]:
  per_case={}
  for state in rows:
   distance=state['camera']['distance']
   assert type(distance)in(int,float)and math.isfinite(distance)and distance>0,'invalid camera distance'
   canonical=float(distance)
   if type(distance)is int:assert int(canonical)==distance,'lossy camera distance'
   state['camera']['distance']=canonical
   if state['case']in per_case:assert state['camera']==per_case[state['case']],'camera changed within case'
   else:per_case[state['case']]=dict(state['camera'])
 equal(a,b,'states')
 assert set(native['controls'])==set(GUARDS)
 for key in GUARDS:assert native['controls'][key]is True,key
 equal(native['native_fault_corrections'],[{'case':'dae-failure-datum','native_atomic_datum':True}],'corrections')
 expected=[c['name']for c in source['cases']if c['saved']is not None];assert expected==[r['case']for r in readers]and len(readers)==11
 for r in readers:equal(r['original'],r['native'],'reader.'+r['case'])
 assert type(native['parses'])is int and native['parses']==12
 assert type(native['context_retired_parses'])is int and native['context_retired_parses']==1
 return {'cases':13,'states':61,'literal_states':60,'declared_native_correction_states':1,'original_readers':11,'operative_guards':len(GUARDS)}
def main():
 source_dir,native_dir,report=map(Path,sys.argv[1:]);source=json.loads((source_dir/'original-exchange.json').read_text());native=json.loads((native_dir/'native-exchange.json').read_text());readers=json.loads((native_dir/'reopens-exchange.json').read_text());result={'passed':False}
 try:
  result.update(compare(source,native,readers));tests=[]
  mutations=[]
  for k in GUARDS:
   n=copy.deepcopy(native);n['controls'][k]=False;mutations.append((k,n,readers))
  for kind in['coordinate','history','datum','reader','fault','inventory','parse','camera','camera-bool','camera-inf','context-parse']:
   n=copy.deepcopy(native);r=copy.deepcopy(readers)
   if kind=='coordinate':n['states'][0]['scene']['active']['points'][0][0]+=1.
   elif kind=='history':n['states'][0]['undo']+=1
   elif kind=='datum':next(s for s in n['states']if s['case']=='dae-failure-datum'and s['phase']=='import')['scene']['datum']={'lat':51.5,'lon':-.1,'alt':32.}
   elif kind=='reader':r.pop()
   elif kind=='fault':n['native_fault_corrections']=[]
   elif kind=='inventory':n['states'].pop()
   elif kind=='parse':n['parses']+=1
   elif kind=='camera':n['states'][0]['camera']['distance']+=1.
   elif kind=='camera-bool':n['states'][0]['camera']['distance']=True
   elif kind=='camera-inf':n['states'][0]['camera']['distance']=float('inf')
   else:n['context_retired_parses']+=1
   mutations.append((kind,n,r))
  for name,n,r in mutations:
   try:compare(source,n,r)
   except(AssertionError,KeyError,TypeError):tests.append(name)
   else:raise AssertionError('Mutation accepted: '+name)
  result['negative_controls']=tests;result['passed']=True
 except BaseException as e:result['error']=str(e)
 finally:report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result));return 0 if result['passed']else 1
if __name__=='__main__':sys.exit(main())
