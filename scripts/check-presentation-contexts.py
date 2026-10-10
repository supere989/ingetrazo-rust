#!/usr/bin/env python3
"""Exact operative source ownership; missing cases and stale recipes fail."""
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text())
required={"ordinary","folded","nested_ordinary","nested_folded"}
def valid(candidate): return set(source)==required and candidate==source
assert valid(native),"presentation context/state mismatch"
negatives=[]
for field in ["style","shadows","undo","redo"]:
 bad=copy.deepcopy(native)
 if field=="style":bad["ordinary"]["states"][1][field]["face_mode"]="shaded"
 elif field=="shadows":bad["folded"]["states"][2][field]["minute"]=0
 else:bad["nested_ordinary"]["states"][1][field]+=1
 assert not valid(bad),field+" negative accepted"
 negatives.append(field)
assert not valid({}),"empty corpus accepted"
print(json.dumps({"passed":True,"cases":4,"states":16,"negative_controls":negatives+["empty"],"boundary":"operative source panels/history versus production app context; no rendered style/shadow proof"},indent=2))
