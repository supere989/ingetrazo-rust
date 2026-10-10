#!/usr/bin/env python3
"""Strict full-frame diagnostic; reports failures without redefining acceptance."""
import json,sys
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage
def pixels(p):
 im=QImage(str(p)).convertToFormat(QImage.Format_RGBA8888)
 assert not im.isNull(),str(p)
 return np.frombuffer(im.constBits(),dtype=np.uint8).reshape(im.height(),im.width(),4).copy()
a,b=map(Path,sys.argv[1:3]);rows=[]
for case in json.loads((a/"original-alpha-results.json").read_text())["cases"]:
 name=case["case"];x=pixels(a/(name+".png"));y=pixels(b/(name+".png"));assert x.shape==y.shape
 delta=np.abs(x.astype(np.int16)-y.astype(np.int16));mask=np.any(delta,axis=2)
 yy,xx=np.nonzero(mask)
 rows.append(dict(case=name,differing_pixels=int(mask.sum()),central_150x150_differing=int(mask[225:375,325:475].sum()),max_channel_difference=int(delta.max()),central_max_channel_difference=int(delta[225:375,325:475].max()),bbox=None if not len(xx) else [int(xx.min()),int(yy.min()),int(xx.max()),int(yy.max())]))
print(json.dumps(rows,indent=2))
raise SystemExit(1 if any(r["differing_pixels"]for r in rows)else 0)
