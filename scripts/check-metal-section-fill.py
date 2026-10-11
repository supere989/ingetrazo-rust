#!/usr/bin/env python3
"""Strict reused-original section pixels; never percentage/shape approximation."""
import argparse,json,hashlib
from pathlib import Path
from PySide6.QtGui import QImage
NAMES=["closed","shared-slab","shared-slab-reversed","transparent","duplicate","coplanar-backface","closed-iso","shared-slab-iso","shared-slab-reversed-iso","closed-front"]
def image(path):
 q=QImage(str(path)).convertToFormat(QImage.Format_RGBA8888);assert not q.isNull() and(q.width(),q.height())==(800,600)
 return bytes(q.constBits())
def validate(root,reference):
 report=json.loads((root/"report.json").read_text());assert [r["case"]for r in report["cases"]]==NAMES
 rows=[]
 for row in report["cases"]:
  name=row["case"];assert row["warm_frames"]==3
  for counter in ["target_allocations","geometry_uploads","section_fill_uploads","section_fill_upload_bytes"]:assert row[counter+"_before"]==row[counter+"_after"]
  source=image(reference/(name+".png"));native=(root/(name+".rgba")).read_bytes();disabled=(root/(name+"-disabled.rgba")).read_bytes();assert len(native)==len(source)==len(disabled)==800*600*4
  diff=sum(source[i:i+4]!=native[i:i+4]for i in range(0,len(source),4))
  mask=lambda b:[b[i]>180 and b[i+2]>180 and b[i+1]<80 for i in range(0,len(b),4)]
  a,b,c=mask(source),mask(native),mask(disabled);mdiff=sum(x!=y for x,y in zip(a,b));assert not any(c),(name,"disabled fill still visible")
  rows.append({"case":name,"rgba_mismatch_pixels":diff,"source_cap_mask_pixels":sum(a),"native_cap_mask_pixels":sum(b),"cap_mask_mismatch_pixels":mdiff,"native_sha256":hashlib.sha256(native).hexdigest()})
 return {"cases":rows,"passed":all(r["rgba_mismatch_pixels"]==0 for r in rows),"scope":"All source/native RGBA pixels, including retained source contour. Native contour presently unimplemented; any mismatch stays failed. Cap mask diagnostics are additional, not a relaxed acceptance."}
def main():
 p=argparse.ArgumentParser();p.add_argument("native",type=Path);p.add_argument("--reference",type=Path,default=Path("docs/audits/section-original-gl"));p.add_argument("--output",type=Path);args=p.parse_args();result=validate(args.native,args.reference);text=json.dumps(result,indent=2)+"\n";print(text)
 if args.output:args.output.write_text(text)
 raise SystemExit(0 if result["passed"]else 1)
if __name__=="__main__":main()
