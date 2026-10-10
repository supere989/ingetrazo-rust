import copy,json,sys
from pathlib import Path
a=json.loads(Path(sys.argv[1]).read_text());b=json.loads(Path(sys.argv[2]).read_text())
def check(a,b):
 assert [x["name"]for x in a]==["selected-curve","ordinary","component","control-toggle"]==[x["name"]for x in b]
 x,y=a[0],b[0]
 for z in [x,y]:
  assert z["fresh_curve"] and z["copied_curve"]!=z["source_curve"] and z["copied_link"] and z["copied_soft"]==[True,True]
 # Generated copied curve IDs differ between independent runtime counters; original source ID remains literal91.
 aa=copy.deepcopy(a);bb=copy.deepcopy(b)
 for z in [aa[0],bb[0]]:
  for attrs in z["copied_attrs"]:
   if "curve"in attrs:assert attrs.pop("curve")==z["copied_curve"]
  z.pop("copied_curve")
 assert aa==bb,(aa,bb)
check(a,b)
for label,mutate in [("curve-collision",lambda d:d[0].update(copied_curve=d[0]["source_curve"])),("child-placement",lambda d:d[1]["world_points"][2].__setitem__(0,99)),("component-unlinked",lambda d:d[2].update(shared_component_mesh=False)),("held-key-rearm",lambda d:d[3].update(press_states=[True,True,True]))]:
 bad=copy.deepcopy(b);mutate(bad)
 try:check(a,bad)
 except AssertionError:pass
 else:raise AssertionError("Negative control accepted: "+label)
print("PASS4 actual source/controller copy cases;4 retained semantic negative controls")
