#!/usr/bin/env python3
"""Boundary visibility only; no shape AA equality claim."""
import json,sys
from pathlib import Path
from PySide6.QtGui import QImage

def check(directory):
 d=Path(directory);r=json.loads((d/'verdict.json').read_text());a=r['start_pixel'];b=r['end_pixel'];out={}
 for phase in ['before','placement-preview','warm-preview','cancelled','committed','undone']:
  im=QImage(str(d/(phase+'.png')))
  assert not im.isNull(),phase+' unreadable PNG'
  assert im.width()==1200 and im.height()==800,phase+' dimensions'
  counts=[]
  for side in range(4):
   hit=0
   for j in range(1,20):
    t=j/20
    x,y=[(a[0]+t*(b[0]-a[0]),a[1]),(b[0],a[1]+t*(b[1]-a[1])),(a[0]+t*(b[0]-a[0]),b[1]),(a[0],a[1]+t*(b[1]-a[1]))][side]
    hit+=any(min(im.pixelColor(round(x)+dx,round(y)+dy).getRgb()[:3])<230 for dx in range(-2,3) for dy in range(-2,3))
   counts.append(hit)
  out[phase]=counts
  if phase in ['placement-preview','warm-preview','committed']:assert min(counts)>=15,(phase,counts)
  else:assert max(counts)==0,(phase,counts)
 return out
if __name__=='__main__':
 print(json.dumps(check(sys.argv[1]),indent=2))
