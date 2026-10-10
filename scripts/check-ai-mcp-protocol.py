#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
def validate(a,b):
 assert isinstance(a,list)and isinstance(b,list)and len(a)==len(b)==9,"Missing protocol cases"
 assert [x["request"].get("id")for x in a]==[1,"tools",0,None,4,5,6,7,8],"Wrong source inventory"
 for records in [a,b]:
  assert records[0]["response"]["result"]["protocolVersion"]=="2024-11-05","Wrong protocol"
  assert {t["name"]for t in records[1]["response"]["result"]["tools"]}=={"run_python","query_model","screenshot","undo","redo"},"Missing or renamed source tool"
 assert a==b,"MCP routing/schema mismatch"
def check(source,native):
 a=json.loads(Path(source).read_text());b=json.loads(Path(native).read_text());validate(a,b)
 negatives=[]
 negatives.append(("empty-native",a,[]));negatives.append(("empty-source",[],b))
 c=copy.deepcopy(b);c[0]["response"]["result"]["protocolVersion"]="wrong";negatives.append(("wrong-protocol",a,c))
 c=copy.deepcopy(b);c[1]["response"]["result"]["tools"][0]["name"]="execute_action";negatives.append(("renamed-python-tool",a,c))
 c=copy.deepcopy(b);c[3]["response"]={};negatives.append(("notification-replied",a,c))
 c=copy.deepcopy(b);c[6]["response"]["result"]["arguments"]["code"]="wrong";negatives.append(("changed-delegation",a,c))
 rejected=[]
 for name,x,y in negatives:
  try:validate(x,y)
  except AssertionError:rejected.append(name)
  else:raise AssertionError("Accepted negative "+name)
 return {"passed":True,"cases":9,"negative_controls_rejected":rejected,"scope":"Pure routing; source tool calls use explicit test stub, not script execution or document edits"}
if __name__=="__main__":print(json.dumps(check(*sys.argv[1:3]),indent=2))
