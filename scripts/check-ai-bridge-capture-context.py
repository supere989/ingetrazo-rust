#!/usr/bin/env python3
"""Strict missing-only context screenshot supplement; reuses frozen observation validators."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from PySide6.QtGui import QImage
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
repo=Path(__file__).resolve().parents[1];helper=repo/"scripts/check-ai-bridge-capture.py"
functions=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef)and n.name in {"exact","owned","image","state"}];assert len(functions)==4
exec(compile(ast.Module(body=functions,type_ignores=[]),str(helper),"exec"),globals())
ap=argparse.ArgumentParser();ap.add_argument("source",type=Path);ap.add_argument("native",type=Path);ap.add_argument("output",type=Path);a=ap.parse_args()
s=json.loads((a.source/"original-ai-bridge-capture.json").read_text());n=json.loads((a.native/"native-ai-bridge-capture.json").read_text())
roster=["startup-billboard-context","shared-texture-context"];assert s["source_pin"]==n["source_pin"]==PIN
assert [c["name"]for c in s["cases"]]==roster and len(s["cases"])==2
assert all(len(c["requests"])==len(c["actions_before"])==3 for c in s["cases"]);exact(s["cases"],n["cases"])
assert s["callback_errors"]==[] and len(s["states"])==len(n["states"])==len(s["render_calls"])==6
controls={"actual_socket","actual_host_renderer","requested_rerender","readonly_model_history_camera","nonuniform_actual_png","worker_joined"}
assert set(n["controls"])==controls and all(type(v)is bool and v for v in n["controls"].values())
for row in s["render_calls"]:assert all(type(row[k])is bool and row[k]for k in ["live_target_restored","live_depth_retained","ordinary_world_rgba_unchanged","scene_readonly"])
rows=[]
for index,(source,native)in enumerate(zip(s["states"],n["states"])):
 try:
  assert source["case"]==roster[index//3]and type(source["index"])is int and source["index"]==index%3
  exact({k:source[k]for k in ["case","index","request"]},{k:native[k]for k in ["case","index","request"]})
  case=s["cases"][index//3]
  for row in [source,native]:
   exact(row["before"],row["after"]);assert row["before"]["edit_context"]==(1 if index%3<2 else 0)
   assert row["reply"]["ok"]is True and set(row["reply"])=={"id","ok","result"}and row["reply"]["id"]==row["request"]["id"]
   assert set(row["reply"]["result"])=={"path","width","height"};assert Path(row["reply"]["result"]["path"]).name=="viewport.png"and Path(row["reply"]["result"]["path"]).parent.name=="ingetrazo-ai"
   exact([row["reply"]["result"]["width"],row["reply"]["result"]["height"]],[640,480]);exact(row["image"]["size"],[640,480])
  exact(state(source["before"],case),state(native["before"],case));assert image(a.source,source)==image(a.native,native),"Strict decoded RGBA differs"
  if index%3==1:
   for report in [s,n]:
    previous=report["states"][index-1]["after"];current=report["states"][index]["before"]
    for key in ["scene","overlays","undo","redo","edit_context"]:exact(previous[key],current[key])
    assert previous["camera"]["yaw"]!=current["camera"]["yaw"]
  rows.append({"case":source["case"],"index":source["index"],"ok":True})
 except(AssertionError,KeyError,TypeError,ValueError)as e:rows.append({"case":source.get("case"),"index":source.get("index"),"ok":False,"error":str(e)})
result={"ok":all(x["ok"]for x in rows),"cases":2,"replies":6,"strict_images":sum(x["ok"]for x in rows),"rows":rows,"negative_controls_accepted":0,"scope":"Strict decoded GPU RGBA and declared per-request readonly state. Runtime epochs/paths only follow frozen validator diagnostic law; no tolerances, pixel fitting, encoding or broader platform/performance claim."}
a.output.write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result));raise SystemExit(0 if result["ok"]else 1)
