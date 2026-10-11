#!/usr/bin/env python3
"""Capture the pinned Scale controller's complete grip and factor geometry."""
import json,sys,types
from pathlib import Path
from PySide6.QtGui import QVector3D as V
from tools.scale import ScaleTool
def xyz(v): return [v.x(),v.y(),v.z()]
boxes=[
 ('volume',[0,0,0],[2,3,4],None,None),
 ('flat',[1.3,.2,-1],[4.3,3.2,-1],None,None),
 ('line',[1,2,3],[1,7,3],None,None),
 ('rotated',[0,0,0],[2,3,4],[3.1,-2.2,.7],[[.8,.6,0],[-.6,.8,0],[0,0,1]]),
 ('reflected-flat',[-1,0,0],[2,4,0],[-2,3,1],[[.8,.6,0],[-.6,.8,0],[0,0,-1]]),
 ('sub-flat',[0,0,0],[1,5e-7,0],None,None)]
point=V(3.125,-1.33,5.5); ray_o=V(5.5,4.25,10);ray_d=V(-.2,-.4,-1).normalized()
rows=[]
for name,lo,hi,origin,axes in boxes:
 t=ScaleTool();t._orig=V(*origin) if origin else None;t._axes=tuple(V(*a) for a in axes) if axes else None
 t._selection_bounds=lambda viewport:(V(*lo),V(*hi))
 viewport=types.SimpleNamespace(scene=types.SimpleNamespace(version=1),_pixel_to_ray=lambda x,y:(ray_o,ray_d))
 t._refresh_box(viewport)
 grips=[]
 for g in t._grips:
  t._grip=g
  for center in [False,True]:
   t.about_center=center;t._anchor=t._anchor_for(g)
   for uniform in [False,True]:
    t.uniform=uniform
    typed=[]
    for values in [[2.],[-1.],[2.,3.],[2.,3.,4.],[0.],[.00001],[.0001]]:
     for absolute in [False,True]:
      value=values[0] if len(values)==1 else tuple(values)
      typed.append({'values':values,'absolute':absolute,'factors':t._typed_factors(value,absolute)})
    grips.append({'params':g.params,'mask':g.mask,'center':center,'uniform':uniform,'position':xyz(t._grip_pos(g)),'anchor':xyz(t._anchor),'point_factors':t._factors_from_point(point),'ray_factors':t._factors_from_cursor(viewport,0,0),'typed':typed})
 t._grip=None;t._hover_grip=None
 state=t.scale_box_state()
 rows.append({'name':name,'low':xyz(t._lo),'high':xyz(t._hi),'origin':xyz(t._orig) if t._orig else None,'axes':[xyz(a)for a in t._axes] if t._axes else None,'grip_count':len(t._grips),'segments':[[xyz(a),xyz(b)]for a,b in state['segments']],'controls':grips})
out={'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','point':xyz(point),'ray_origin':xyz(ray_o),'ray_direction':xyz(ray_d),'cases':rows,'scope':'Actual source Scale grip-box/controller math, not native app or rendered input acceptance.'}
Path(sys.argv[1]).write_text(json.dumps(out,indent=2)+'\n')
print(len(rows),'boxes',sum(len(r['controls'])for r in rows),'grip/modifier controls')
