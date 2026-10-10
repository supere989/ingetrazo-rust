#!/usr/bin/env python3
"""Original colored-face style/depth/blend protocol; edges/textures separately gated."""
import json,sys
from pathlib import Path
source,native,baseline=[json.loads(Path(p).read_text())["cases"] for p in sys.argv[1:4]]
required={f"style-{mode}-{kind}" for mode in ["wireframe","hidden_line","monochrome","xray","shaded"] for kind in ["front","reverse","translucent","explicit-back"]}
assert required <= {c["case"] for c in source}
assert len(source)==len(native)>=30
old={c["case"]:c for c in baseline};failed=0
for a,b in zip(source,native):
 assert a["case"]==b["case"] and a["center_rgba"]==b["center_rgba"],(a["case"],a["center_rgba"],b["center_rgba"])
 assert b["geometry_uploads"]==1
 if a["case"] in required:failed+=max(abs(x-y)for x,y in zip(a["center_rgba"],old[a["case"]]["center_rgba"]))>8
assert failed>=14,"old forced-shaded style negative controls must fail"
print(f"PASS {len(source)} exact RGBA controls, uniform-only style/cache assertions; {failed} retained old failures. Edge/profile/texture style workflows remain open.")
