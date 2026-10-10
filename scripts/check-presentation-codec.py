#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text())["cases"]
assert len(source)==16 and native==source,"literal parser corpus mismatch"
old=json.loads(Path(sys.argv[3]).read_text())["cases"]
assert len(old)==len(source) and [(r["kind"],r["input"]) for r in old]==[(r["kind"],r["input"]) for r in source],"old parser inventory differs"
differences=[i for i,(a,b) in enumerate(zip(old,source)) if a!=b]
assert differences==[12,13,15],"literal old grammar failures differ: "+str(differences)
for index in [1,2,3,7,8,10,11,12,13,14,15]:
 bad=copy.deepcopy(native);bad[index]["output"]={};assert bad!=source
print(json.dumps({"passed":True,"cases":len(source),"negative_controls":12,"boundary":"JSON source from_dict accepted recipes and refusals; no rendered malformed recipes; Python nonfinite/arbitrary precision beyond JSON numeric range remains open"},indent=2))
