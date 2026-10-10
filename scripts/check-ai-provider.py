#!/usr/bin/env python3
"""Strict offline corpus validation, with negatives using the same validator."""
import copy,itertools,json,sys
from pathlib import Path
COUNTS={"requests":14,"detect":8,"vision":6,"code":5,"compact":3,"thoughts":5}
PROVIDERS={"groq","anthropic","openai","gemini","openrouter","deepseek","ollama"}
def validate(original,native):
 for label,data in [("original",original),("native",native)]:
  assert isinstance(data,dict)and set(data)==set(COUNTS),f"{label}: missing/extra sections"
  assert all(isinstance(data[k],list)and len(data[k])==n for k,n in COUNTS.items()),f"{label}: missing corpus cases"
  rows=data["requests"]
  assert all(isinstance(r,dict)and isinstance(r.get("provider"),str)and isinstance(r.get("vision"),bool)for r in rows),f"{label}: malformed request inventory"
  assert {(r["provider"],r["vision"])for r in rows}==set(itertools.product(PROVIDERS,[True,False])),f"{label}: incomplete provider/vision inventory"
 assert original==native,"Provider wire/schema mismatch"
def check(original,native):
 a=json.loads(Path(original).read_text());b=json.loads(Path(native).read_text());validate(a,b);negatives=[]
 for section in COUNTS:
  bad=copy.deepcopy(b);bad[section][0]="MUTATED";negatives.append(("mutated-"+section,a,bad))
 bad=copy.deepcopy(b);bad["requests"][0]=copy.deepcopy(bad["requests"][2]);negatives.append(("duplicate-provider-inventory",a,bad))
 bad=copy.deepcopy(b);bad["requests"]=[];negatives.append(("empty-native-corpus",a,bad))
 bad=copy.deepcopy(a);bad["requests"]=[];negatives.append(("empty-original-corpus",bad,b))
 bad=copy.deepcopy(b);bad.pop("code");negatives.append(("missing-section",a,bad))
 bad=copy.deepcopy(b);bad["requests"][0]["payload"]["model"]="wrong";negatives.append(("changed-model",a,bad))
 rejected=[]
 for name,source,candidate in negatives:
  try:validate(source,candidate)
  except AssertionError:rejected.append(name)
  else:raise AssertionError("Validator accepted negative "+name)
 return {"passed":True,"cases":sum(COUNTS.values()),"negative_controls_rejected":rejected,"scope":"Offline pinned source provider wire; no network/executor/MCP behavior"}
if __name__=="__main__":print(json.dumps(check(*sys.argv[1:3]),indent=2))
