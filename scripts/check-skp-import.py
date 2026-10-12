#!/usr/bin/env python3
"""Exact finite SKP App workflow and actual original save-reader comparison."""
import sys,json,copy,math,importlib.util
from pathlib import Path
spec=importlib.util.spec_from_file_location('exchange_strict',Path(__file__).with_name('check-exchange-import.py'));strict=importlib.util.module_from_spec(spec);spec.loader.exec_module(strict)
equal=strict.equal
NAMES=['legacy-root','modern-layer-preserve','saved-scenes','shared-root','nested-root-ownership','leader-metadata','image-leaf','style-adopt','style-existing','material-collision','repeat-import','modern-empty','legacy-empty-warning','unreadable-warning','chooser-cancel']
GUARDS=['actual_chooser','file_worker','parse_once','camera_unchanged','atomic_refusal','idle_readonly','context_retired','registry_undo']
def compare(source,native,readers):
 assert source['source_pin']==native['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert source['callback_errors']==[]
 assert [c['name']for c in source['cases']]==NAMES
 equal([{k:c[k]for k in['name','saved','phases']}for c in source['cases']],native['cases'],'cases')
 a=copy.deepcopy(source['states']);b=copy.deepcopy(native['states']);assert len(a)==len(b)==71
 for rows in[a,b]:
  cameras={}
  for state in rows:
   distance=state['camera']['distance']
   assert type(distance)in(int,float)and math.isfinite(distance)and distance>0
   number=float(distance)
   if type(distance)is int:assert int(number)==distance
   state['camera']['distance']=number
   if state['case']in cameras:equal(state['camera'],cameras[state['case']],'camera unchanged')
   else:cameras[state['case']]=dict(state['camera'])
 equal(a,b,'states')
 assert set(native['controls'])==set(GUARDS)
 for key in GUARDS:assert native['controls'][key]is True,key
 names=[c['name']for c in source['cases']if c['saved']is not None];assert len(names)==12 and names==[r['case']for r in readers]
 for row in readers:equal(row['original'],row['native'],'reader.'+row['case'])
 assert type(native['parses'])is int and native['parses']==source['parses']==14
 assert type(native['context_retired_parses'])is int and native['context_retired_parses']==1
 return {'cases':15,'states':71,'original_readers':12,'operative_guards':8}
def main():
 src,out,report=map(Path,sys.argv[1:]);source=json.loads((src/'original-skp.json').read_text());native=json.loads((out/'native-skp.json').read_text());readers=json.loads((out/'reopens-skp.json').read_text());result={'passed':False}
 try:
  result.update(compare(source,native,readers));mutations=[]
  for key in GUARDS:
   n=copy.deepcopy(native);n['controls'][key]=False;mutations.append((key,n,readers))
  for kind in['coordinate','history','reader','inventory','parse','camera','camera-bool','camera-inf','context-parse','axes','owner','material','back','layers','view','dimension','text']:
   n=copy.deepcopy(native);r=copy.deepcopy(readers)
   if kind=='coordinate':n['states'][0]['scene']['active']['points'][0][0]+=1.
   elif kind=='history':n['states'][0]['undo']+=1
   elif kind=='reader':r.pop()
   elif kind=='inventory':n['states'].pop()
   elif kind=='parse':n['parses']+=1
   elif kind=='camera':n['states'][0]['camera']['distance']+=1.
   elif kind=='camera-bool':n['states'][0]['camera']['distance']=True
   elif kind=='camera-inf':n['states'][0]['camera']['distance']=float('inf')
   elif kind=='context-parse':n['context_retired_parses']+=1
   elif kind=='axes':n['states'][0]['scene']['groups'][0]['axes'][0]+=1.
   elif kind=='owner':n['states'][0]['scene']['groups'][0]['children'][0]['mesh_owner']=999
   elif kind=='material':next(s for s in n['states']if s['case']=='material-collision'and s['phase']=='undo')['scene']['materials'].pop('Green (2)')
   elif kind=='back':next(s for s in n['states']if s['case']=='style-adopt'and s['phase']=='undo')['scene']['back_face_color']=None
   elif kind=='layers':n['states'][0]['scene']['layers'][0]['visible']=False
   elif kind=='view':n['states'][0]['scene']['views'][0]['distance']+=1.
   elif kind=='dimension':next(s for s in n['states']if s['case']=='leader-metadata'and s['phase']=='import')['scene']['dimensions'].pop()
   else:next(s for s in n['states']if s['case']=='leader-metadata'and s['phase']=='import')['scene']['texts'][0]['text']='Changed'
   mutations.append((kind,n,r))
  negatives=[]
  for name,n,r in mutations:
   try:compare(source,n,r)
   except(AssertionError,KeyError,TypeError):negatives.append(name)
   else:raise AssertionError('mutation accepted '+name)
  result['negative_controls']=negatives;result['passed']=True
 except BaseException as e:result['error']=str(e)
 finally:report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result));return 0 if result['passed']else 1
if __name__=='__main__':sys.exit(main())
