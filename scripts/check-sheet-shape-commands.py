import json,sys,math
from pathlib import Path
a=json.loads(Path(sys.argv[1]).read_text());b=json.loads(Path(sys.argv[2]).read_text());assert len(a)==len(b)==9
checks=0
def equal(x,y):
 global checks
 if isinstance(x,(float,int)) and isinstance(y,(float,int)):assert math.isclose(x,y,rel_tol=0,abs_tol=1e-9),(x,y);checks+=1
 elif isinstance(x,dict):assert x.keys()==y.keys();[equal(x[k],y[k])for k in x]
 elif isinstance(x,list):assert len(x)==len(y);[equal(v,w)for v,w in zip(x,y)]
 else:assert x==y,(x,y);checks+=1
for source,native in zip(a,b):equal(source['commands'],native)
print(json.dumps({'passed':True,'cases':9,'leaf_checks':checks,'boundary':'Actual original painter draw geometry/palette vs native physical commands; source PNG retained, native preview rasterization/clip-cap pixel equivalence and exports remain open'}))
