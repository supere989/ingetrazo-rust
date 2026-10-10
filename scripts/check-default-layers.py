#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
NAMES=["omitted","empty","authored-Layer0","authored-Untagged","explicit-default","authored-first","missing-name"]
def validate(a,b):
 for data in (a,b):
  assert set(data)=={"new_scene","cases","assignment"},"top-level inventory"
  assert [c["name"]for c in data["cases"]]==NAMES,"case inventory"
  assert len(data["assignment"])==5,"history inventory"
 assert a==b,"native default registry/reference mismatch"
def main():
 p=Path(sys.argv[1]);a=json.loads((p/"original.json").read_text());b=json.loads((p/"native.json").read_text());validate(a,b)
 negatives=[]
 for name in ["old-default","rename-authored","wrong-default-visibility","lose-default-clearing","missing-case"]:
  x=copy.deepcopy(b)
  if name=="old-default":x["new_scene"]["names"]=["Layer0"]
  elif name=="rename-authored":x["cases"][2]["names"]=["Layer 0"]
  elif name=="wrong-default-visibility":x["cases"][5]["default_visible"]=False
  elif name=="lose-default-clearing":x["assignment"][2]["edge"]="Layer 0"
  else:x["cases"].pop()
  try:validate(a,x)
  except AssertionError:negatives.append(name)
  else:raise AssertionError("negative accepted: "+name)
 print(json.dumps({"equal":True,"codec_cases":7,"assignment_states":5,"negative_rejections":negatives},indent=2))
if __name__=="__main__":main()
