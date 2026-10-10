import copy,json,sys,math
from pathlib import Path
EXPECTED=["rect","rounded","ellipse","polygon-clamped","line-inverted","arrow","terrain-ticks","terrain-hatch-inverted","terrain-band"]
def validate(source,native):
 assert [r["name"]for r in source]==EXPECTED,"Missing, duplicate or reordered original shape recipe"
 assert len(native)==len(EXPECTED),"Missing or extra native shape"
 checks=0
 def equal(x,y):
  nonlocal checks
  if type(x)in(float,int)and type(y)in(float,int):assert math.isclose(x,y,rel_tol=0,abs_tol=1e-9),(x,y);checks+=1
  elif isinstance(x,dict):assert isinstance(y,dict)and x.keys()==y.keys();[equal(x[k],y[k])for k in x]
  elif isinstance(x,list):assert isinstance(y,list)and len(x)==len(y);[equal(v,w)for v,w in zip(x,y)]
  else:assert type(x)is type(y)and x==y,(x,y);checks+=1
 for reference,commands in zip(source,native):equal(reference["commands"],commands)
 return checks
source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());checks=validate(source,native)
negatives=[]
for mode in ["missing","duplicated-recipe","geometry","palette","opacity"]:
 a,b=copy.deepcopy(source),copy.deepcopy(native)
 if mode=="missing":b.pop()
 elif mode=="duplicated-recipe":a[-1]=a[0]
 elif mode=="geometry":b[3][0]["path"]["polygon"][0][0]+=1.
 elif mode=="palette":b[1][0]["fill"][0]="#ffffff"
 else:b[-1][0]["fill"][1]=1.
 try:validate(a,b)
 except (AssertionError,TypeError,KeyError):negatives.append(mode)
 else:raise AssertionError("Negative control accepted: "+mode)
print(json.dumps({"passed":True,"cases":9,"leaf_checks":checks,"negative_controls":negatives,"geometry_absolute_tolerance":1e-9,"boundary":"Actual original painter draw geometry/palette vs native physical commands; source PNG retained, native preview rasterization/clip-cap pixel equivalence and exports remain open"}))
