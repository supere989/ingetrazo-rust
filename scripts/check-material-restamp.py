#!/usr/bin/env python3
"""Strict retained material law, current App, tray and original-reader checks."""
import argparse,copy,json,zipfile
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
NAMES=["root","classic","placed","nested"]
PHASES=["before","edited","undo","redo","finished","finished-undo","finished-redo"]
def validate(source,native,tray,reopened,expected):
 assert source["source_pin"]==PIN and reopened["source_pin"]==PIN
 assert [c["case"] for c in source["cases"]]==NAMES
 assert [c["case"] for c in native["cases"]]==NAMES
 exact=0
 for s,n in zip(source["cases"],native["cases"]):
  phases=PHASES[:4] if s["case"]=="root" else PHASES
  assert [p["phase"] for p in s["phases"]]==phases
  assert [p["phase"] for p in n["phases"]]==phases
  for a,b in zip(s["phases"],n["phases"]):
   if a["phase"]=="finished-undo" and s["case"] in ["placed","nested"]:
    # Literal source detached-target bug retained; exact native integrity result
    # is authenticated separately, including context-specific UVW frames.
    assert a!=b
   else:assert a==b,(s["case"],a["phase"]);exact+=1
 assert native==expected["native"],"native integrity/ownership payload changed"
 assert tray==expected["tray"] and reopened==expected["reopened"]
 p=tray["phases"];assert [x["phase"] for x in p]==["before","cancelled","edited","undo","redo"]
 assert {k:v for k,v in p[0].items() if k!="phase"}=={k:v for k,v in p[1].items() if k!="phase"}
 assert [(x["undo"],x["redo"],x["revision"]) for x in p]==[(0,0,0),(0,0,0),(1,0,1),(0,1,2),(1,0,3)]
 assert [x["case"] for x in reopened["cases"]]==NAMES+["tray"]
 for case in reopened["cases"]:
  state=case["state"];assert state["registry"]["Oak"]["name"]=="Oak"
  assert "texture" not in state["registry"]["Oak"]
  for f in state["loose"]:assert "back" in f and "texture" not in f
 return {"exact_source_states":exact,"explicit_source_integrity_divergences":2,"tray_phases":5,"original_archive_reopens":5}
def main():
 p=argparse.ArgumentParser();p.add_argument("--archive",type=Path,default=Path("docs/audits/material-edit-restamp/records.zip"));p.add_argument("--native",type=Path);p.add_argument("--tray",type=Path);a=p.parse_args()
 with zipfile.ZipFile(a.archive) as z:
  get=lambda n:json.loads(z.read(n));s=get("source.json");n=get("native.json");t=get("tray.json");r=get("reopened.json");expected={"native":copy.deepcopy(n),"tray":copy.deepcopy(t),"reopened":copy.deepcopy(r)}
 if a.native:n=json.loads(a.native.read_text())
 if a.tray:t=json.loads(a.tray.read_text())
 result=validate(s,n,t,r,expected)
 negatives=[]
 for name,mutate in [("empty",lambda x:x["cases"].clear()),("color-drift",lambda x:x["cases"][0]["phases"][1]["active"][0]["color"].__setitem__(0,.61)),("lost-back",lambda x:x["cases"][0]["phases"][1]["active"][0].pop("back")),("lost-opaque",lambda x:x["cases"][0]["phases"][1]["active"][0].pop("vendor")),("retained-old-texture",lambda x:x["cases"][0]["phases"][1]["active"][0].__setitem__("texture",{})),("missing-phase",lambda x:x["cases"][1]["phases"].pop())]:
  bad=copy.deepcopy(n);mutate(bad)
  try:validate(s,bad,t,r,expected)
  except (AssertionError,KeyError,IndexError):negatives.append(name)
  else:raise AssertionError("accepted mutation "+name)
 result["rejected_controls"]=negatives;print(json.dumps(result,indent=2))
if __name__=="__main__":main()
