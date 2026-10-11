#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-units.json').read_text());b=json.loads((p/'native-units.json').read_text())
phases=['loaded-composite','cancel','unchanged']+['apply-'+c for c in ['m','cm','mm','in','ft','ft-in','in-frac','ft-in-frac']]+['restart-new','open-retains-document','unchecked-default','malformed-new']
def exact(a,b):
 if type(a)is not type(b):return False
 if isinstance(a,dict):return a.keys()==b.keys()and all(exact(a[k],b[k])for k in a)
 if isinstance(a,list):return len(a)==len(b)and all(exact(x,y)for x,y in zip(a,b))
 return a==b
def check(candidate):
 assert [r['phase']for r in candidate['rows']]==phases
 assert exact(a,candidate),'source/native units state mismatch'
check(b)
mutations=[(0,'units','length','m'),(1,'default','length','mismatch'),(2,'style','units','ft-in'),(3,'style','vendor',None),(8,'units','length','m'),(10,'default','precision',2),(11,'style','decimals',2),(12,'units','length','ft-in-frac'),(13,'default','length','cm'),(14,'units','length','cm')]
for row,field,key,value in mutations:
 bad=copy.deepcopy(b);bad['rows'][row][field][key]=value
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError('mutation accepted')
print(json.dumps({'states':len(phases),'negatives':len(mutations),'typed_exact':True}))
