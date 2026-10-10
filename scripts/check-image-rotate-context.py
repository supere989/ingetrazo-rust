import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
required={c+":"+f for c in ["plain","component","folded","nested","nested_folded"]for f in ["finish_live","cancel_then_finish"]}
assert set(source)==set(native)==set(old)==required,"missing required context family"
assert source==native,"source active Rotate context/frame/history mismatch"
for k in required:
 if k.endswith(":finish_live"):assert source[k]!=old[k],"old cancellation not rejected"
 else:assert source[k]==old[k],"cancel-positive source control changed"
for field in ["frame","active","undo"]:
 bad=copy.deepcopy(native);row=bad["folded:finish_live"][3]
 if field=="frame":row[field][1][0]+=1
 elif field=="active":row[field]=not row[field]
 else:row[field]+=1
 assert bad!=source,"mutation accepted"
print(json.dumps({"cases":10,"states":sum(map(len,source.values())),"exact":True,"old_failures":5,"old_positives":5,"mutations_rejected":3}))
