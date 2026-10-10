#!/usr/bin/env python3
import json,copy,sys
from pathlib import Path
def validate(original,native):
 for key in ["captured_stdout","persistent_value","syntax_refused","runtime_exception"]:assert original[key]==native[key],key
 assert native["engine"]=="RustPython0.6.0"
 assert all(native[k] is True for k in ["persistent_scope","functions_classes_loops","native_module","eval_exec"])
 assert native["scene_adapters"] is False and native["atomic_journal"] is False
 for d in (original,native):
  assert 'File "<ai>", line 1' in d["traceback"]
  assert d["traceback"].endswith("ValueError: literal-failure\n")
 assert original["unchanged_history"] is True and original["changed_flags"]==[False,False,False]
p=Path(sys.argv[1]);a=json.loads((p/"original.json").read_text());b=json.loads((p/"native.json").read_text());validate(a,b);rejected=[]
for key,value in [("captured_stdout",""),("persistent_value","570"),("syntax_refused",False),("traceback","no error"),("atomic_journal",True)]:
 x=copy.deepcopy(b);x[key]=value
 try:validate(a,x)
 except AssertionError:rejected.append(key)
 else:raise AssertionError("mutation accepted: "+key)
print(json.dumps({"language_contract_equal":True,"full_traceback_bytes_equal":a["traceback"]==b["traceback"],"native_extension_control":"synthetic fixture module, not CAD adapter","negative_rejections":rejected,"scene_journal_implemented":False},indent=2))
