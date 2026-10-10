#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
def norm(v):
 if isinstance(v,dict):return{k:norm(x)for k,x in v.items()}
 if isinstance(v,list):return[norm(x)for x in v]
 return "Layer 0" if v=="Layer0" else v
def accepts(v):
 if set(v)!=set(source):return False
 for name,states in source.items():
  if len(v[name])!=4:return False
  first={k:value for k,value in states[0].items() if k not in ["outside","sibling"]}
  if norm(v[name][0])!=first or norm(v[name][1:])!=states[1:]:return False
 return True
native=json.loads(Path(sys.argv[2]).read_text());assert accepts(native),(source,native)
if len(sys.argv)>3:
 old=json.loads(Path(sys.argv[3]).read_text());assert not accepts(old);assert old["single"][1]["outside"]=="Tagged" and norm(old["single"][1]["outside"])!=source["single"][1]["outside"]
for mutation in ["restore_outer_tag","restore_shared_old_tag","remove_single_old_tag"]:
 bad=copy.deepcopy(native)
 if mutation=="restore_outer_tag":bad["single"][2]["outside"]="Tagged"
 elif mutation=="restore_shared_old_tag":bad["linked"][2]["inside"]="Tagged"
 else:bad["single"][2]["inside"]="Layer0"
 assert not accepts(bad)
print(json.dumps({"passed":True,"cases":4,"states":16,"negatives":3,"scope":"entry local layers/mesh/history plus full finished Undo/Redo state; parked outside/sibling not observed during native entry; no rendered input proof"},indent=2))
