#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text())
assert set(source)=={"ordinary","folded","nested_ordinary","nested_folded"} and native==source,"operative image planes/context/history mismatch"
baseline=json.loads(Path(sys.argv[3]).read_text());assert baseline["runtime_planes"] is None,"literal baseline not missing modeled planes"
for key in ["selected_images","planes","layers","undo","redo"]:
 bad=copy.deepcopy(native);row=bad["ordinary"][1];row[key]=[] if key in ["selected_images","planes","layers"]else 99;assert bad!=source
assert {}!=source
print(json.dumps({"passed":True,"states":sum(map(len,source.values())),"negative_controls":7,"boundary":"actual pinned image commands versus app-controller valid reference model/history; GPU picking/draw/UI image-tool coverage separate"},indent=2))
