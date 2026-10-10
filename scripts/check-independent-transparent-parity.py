#!/usr/bin/env python3
"""Literal original/native alpha comparison; retained open scope never means parity passed."""
import argparse,json
from pathlib import Path
CASES={"red-half-white","red-half-black","opacity-bucket-order","opaque-front","opaque-back","equal-opacity-forward","equal-opacity-reversed","opacity-995","opacity-zero","section-clipped"}
KNOWN_OPEN=CASES-{"opaque-front","opacity-zero"}
def compare(original,native):
 def rows(value):
  items=value.get("cases",[])
  names=[r.get("case")for r in items]
  if len(names)!=10 or set(names)!=CASES or len(set(names))!=10:raise ValueError("Missing/duplicate mandatory ten alpha cases")
  return {r["case"]:r for r in items}
 a,b=rows(original),rows(native);result=[]
 for name in sorted(CASES):
  left=a[name]["center_rgba"];right=b[name]["center_rgba"]
  if len(left)!=4 or len(right)!=4:raise ValueError("Malformed RGBA control")
  result.append({"case":name,"original":left,"native":right,"passed":left==right})
 return {"passed":all(r["passed"]for r in result),"cases":result,"scope":"Ten original colored untextured front-surface center pixels. Texture/backside/instancing and general alpha remain open."}
def main():
 p=argparse.ArgumentParser();p.add_argument("original");p.add_argument("native");p.add_argument("output");p.add_argument("--retain-open",action="store_true");a=p.parse_args()
 left=json.loads(Path(a.original).read_text());right=json.loads(Path(a.native).read_text())
 result=compare(left,right);failed={r["case"]for r in result["cases"]if not r["passed"]}
 # Negative controls cover omitted corpus and a corrupted opaque surface.
 try:compare({"cases":left["cases"][:-1]},right)
 except ValueError:pass
 else:raise AssertionError("Missing alpha case accepted")
 bad=json.loads(json.dumps(right));r=next(r for r in bad["cases"]if r["case"]=="opaque-front");r["center_rgba"]=[255,0,0,255]
 assert not next(r for r in compare(left,bad)["cases"]if r["case"]=="opaque-front")["passed"]
 result["negative_controls_passed"]=True;result["retained_known_open"]=a.retain_open and failed==KNOWN_OPEN
 Path(a.output).write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result,indent=2))
 return 0 if result["passed"]or result["retained_known_open"]else 1
if __name__=="__main__":raise SystemExit(main())
