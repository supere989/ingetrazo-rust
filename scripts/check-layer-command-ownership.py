#!/usr/bin/env python3
"""Strict source/native layer states; explicit legacy default-label normalization."""
import json,copy,sys
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text())
def norm(value):
 if isinstance(value,list):return [norm(v) for v in value]
 if isinstance(value,dict):return {k:norm(v) for k,v in value.items()}
 return "Layer 0" if value=="Layer0" else value
def accepts(rows):return norm(rows)==source
assert accepts(native),(source,native)
negative={}
for name in ["drop_panel_layer","stale_purge_visibility","remove_renamed_assignment","lose_import_object"]:
 bad=copy.deepcopy(native)
 if name=="drop_panel_layer":bad["panel_state_across_model_history"][1]["layers"].pop()
 elif name=="stale_purge_visibility":bad["purge_held_object_visibility"][-1]["layers"][-1][1]=True
 elif name=="remove_renamed_assignment":bad["assign_created_rename_lookup"][1]["layers"].pop()
 else:bad["import_deleted_held_object"][-1]["layers"].pop()
 assert not accepts(bad);negative[name]="rejected"
if len(sys.argv)>3:
 old=json.loads(Path(sys.argv[3]).read_text());assert not accepts(old)
print(json.dumps({"passed":True,"cases":4,"states":13,"negative_controls":negative,"default_label_normalization":"native legacy Layer0 -> source Layer 0; all other strings exact","scope":"operative original Qt panel/History and native app controller; no rendered input proof or full layer parity"},indent=2))
