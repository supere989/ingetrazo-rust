#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-import-preferences.json').read_text());b=json.loads((p/'native-import-preferences.json').read_text())
cases=['obj-cancel','obj-stored','obj-size','obj-huge','obj-explicit','dxf-header','dxf-lying','dxf-stored']
roster=[(c,s)for c in cases for s in (['confirmed']if c=='obj-cancel'else['confirmed','import','undo','redo'])]
def exact(a,b):
 if type(a)is not type(b):return False
 if isinstance(a,dict):return a.keys()==b.keys()and all(exact(a[k],b[k])for k in a)
 if isinstance(a,list):return len(a)==len(b)and all(exact(x,y)for x,y in zip(a,b))
 return a==b
def check(x):
 assert [(r['case'],r['phase'])for r in x['rows']]==roster
 assert [r['phase']for r in x['preferences']]==['default','draft','cancel','accept','restart']
 assert exact(a,x),'source/native import preference mismatch'
check(b)
negatives=0
for field in ['suggested','factor','obj','dxf','points','undo','redo']:
 bad=copy.deepcopy(b);row=bad['rows'][2]
 if field in ['undo','redo']:row[field]+=1
 elif field=='factor':row[field]*=100
 elif field=='points':row[field][0][0]+=1.
 else:row[field]='wrong'
 try:check(bad)
 except AssertionError:negatives+=1
 else:raise AssertionError(field)
for i in [1,2,4]:
 bad=copy.deepcopy(b);bad['preferences'][i]['obj']='mm'
 try:check(bad)
 except AssertionError:negatives+=1
 else:raise AssertionError(i)
print(json.dumps({'states':len(roster),'preferences':5,'negatives':negatives,'typed_exact':True}))
