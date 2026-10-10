#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
assert set(source)=={"rename","existing_redo","delete","nested_delete"}
assert native==source,"original image/layer command ownership mismatch"
assert old!=source,"literal old candidate must reject"
failures=[(k,i)for k in source for i,(a,b)in enumerate(zip(source[k],old[k]))if a!=b]
assert {k for k,i in failures}==set(source),"old failure coverage missing"
for key in ["layers","planes","undo","redo"]:
 bad=copy.deepcopy(native);bad["rename"][0][key]=[]if key in ["layers","planes"]else 99;assert bad!=source
assert {}!=source
print(json.dumps({"passed":True,"states":sum(map(len,source.values())),"negative_controls":6,"literal_old_failures":failures,"boundary":"controller reference membership/global layer clear; image graphics and remaining command families open"},indent=2))
