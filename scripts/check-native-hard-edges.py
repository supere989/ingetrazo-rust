#!/usr/bin/env python3
"""Compare literal original GL hard-edge fixture images; exact RGB, no tolerance."""
import argparse,json
from pathlib import Path
from PySide6.QtGui import QImage
parser=argparse.ArgumentParser();parser.add_argument("original");parser.add_argument("native");parser.add_argument("--output");args=parser.parse_args()
mandatory=["style-wireframe-asymmetric-line","style-wireframe-reflected-line","style-wireframe-hidden-group","style-wireframe-hidden-edge","style-wireframe-hidden-layer","style-wireframe-soft-line","style-wireframe-clipped-line","style-hidden_line-occluded-line","style-xray-occluded-line"]
original=Path(args.original);native=Path(args.native);source=json.loads((original/"original-alpha-results.json").read_text());actual=json.loads((native/"native-alpha-results.json").read_text());source_names={r["case"] for r in source["cases"]};actual_names={r["case"] for r in actual["cases"]};rows=[]
for name in mandatory:
 assert name in source_names and name in actual_names,"Missing mandatory fixture "+name
 a=QImage(str(original/(name+".png")));b=QImage(str(native/(name+".png")));assert not a.isNull() and not b.isNull(),"Missing fixture image "+name;assert a.size()==b.size(),"Image size mismatch "+name
 differing=sum(a.pixel(x,y)!=b.pixel(x,y) for y in range(a.height()) for x in range(a.width()));rows.append(dict(case=name,whole_rgba_differing_pixels=differing,pass_exact=differing==0))
result=dict(scope="Exact software/hardware image corpus only; profiles/textures/back-edge dashes/platform/performance acceptance remains separate",cases=rows,pass_all=all(r["pass_exact"] for r in rows));text=json.dumps(result,indent=2);print(text)
if args.output:Path(args.output).write_text(text+"\n")
raise SystemExit(0 if result["pass_all"] else 1)
