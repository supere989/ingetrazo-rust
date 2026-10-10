#!/usr/bin/env python3
"""Strict actual production CPU Canvas tile comparison; no GPU/WSI claim."""
import argparse,json,pathlib,hashlib
import numpy as np
from PySide6.QtGui import QImage
NAMES=("helvetica","helvetica-bold","helvetica-italic","times-bold-italic","cjk","arabic")
def pixels(path):
 im=QImage(str(path)).convertToFormat(QImage.Format.Format_RGBA8888)
 if im.isNull() or (im.width(),im.height())!=(800,300):raise ValueError("Missing or wrong-sized required image")
 return np.frombuffer(im.constBits(),dtype=np.uint8).reshape(300,im.bytesPerLine())[:,:3200].reshape(300,800,4).copy()
def compare(a,b):
 if not np.any(a[:,:,:3]!=255) or not np.any(b[:,:,:3]!=255):raise ValueError("Blank required image")
 n=int(np.any(a!=b,axis=2).sum())
 if n:raise ValueError(f"{n} changed pixels")
def main():
 ap=argparse.ArgumentParser();ap.add_argument("source",type=pathlib.Path);ap.add_argument("native",type=pathlib.Path);ap.add_argument("output",type=pathlib.Path);a=ap.parse_args()
 source=json.loads((a.source/"original-macos-fonts.json").read_text())
 if source.get("source_pin")!="6be29fe437a7cd992d4e079f71720821b25216b9" or source.get("qt")!="6.12.0":raise ValueError("Frozen original source/Qt provenance differs")
 source_rows=source.get("cases",[])
 if len(source_rows)!=6 or sorted(x["name"] for x in source_rows)!=sorted(NAMES):raise ValueError("Original six-case inventory differs")
 for r in source_rows:
  if hashlib.sha256((a.source/f"{r['name']}.png").read_bytes()).hexdigest()!=r["png_sha256"]:raise ValueError("Original capture hash differs")
 report=json.loads((a.native/"native-canvas.json").read_text());rows=report.get("cases",[])
 if len(rows)!=6 or sorted(x["name"] for x in rows)!=sorted(NAMES):raise ValueError("Required six-case inventory differs")
 out=[]
 for name in NAMES:
  row=next(x for x in rows if x["name"]==name)
  try:
   if row.get("passed") is not True or row.get("warm_unchanged") is not True or row.get("warm_frames")!=3:raise ValueError("Required production case refused or warm proof missing")
   src=pixels(a.source/f"{name}.png");dst=pixels(a.native/f"{name}.native.png");compare(src,dst)
   controls={}
   for kind in ("blank","one-pixel"):
    bad=dst.copy()
    if kind=="blank":bad[:]=255
    else:bad[0,0,0]^=1
    try:compare(src,bad)
    except ValueError:controls[kind]=True
    else:raise ValueError("Mutation unexpectedly accepted")
   out.append(dict(name=name,passed=True,changed_pixels=0,controls=controls))
  except Exception as e:out.append(dict(name=name,passed=False,error=str(e)))
 result=dict(passed=all(x["passed"] for x in out),scope="Actual production CPU native Canvas cached tile + encoded white composition, not GPU/WSI/physical desktop acceptance",cases=out)
 a.output.write_text(json.dumps(result,indent=2));return 0 if result["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
