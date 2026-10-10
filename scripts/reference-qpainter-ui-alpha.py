#!/usr/bin/env python3
"""Actual Qt raster composition, independent of egui's Color32 representation."""
import json,sys
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtCore import QRectF
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
rgba=[[255,0,0,255],[0,255,0,255],[0,0,255,255],[255,255,255,255]]
alpha=[[255,0,0,128],[0,255,0,128],[0,0,255,128],[255,255,255,128]]
cases=[{'case':'solid-red64-black','color':[255,0,0,64],'background':[0,0,0]}, {'case':'solid-red128-black','color':[255,0,0,128],'background':[0,0,0]}, {'case':'solid-red192-black','color':[255,0,0,192],'background':[0,0,0]}, {'case':'solid-rgb128-white','color':[37,93,157,128],'background':[255,255,255]}, {'case':'solid-white128-black','color':[255,255,255,128],'background':[0,0,0]}, {'case':'solid-blue128-gray','color':[0,0,255,128],'background':[64,64,64]}, {'case':'opaque-rgb','color':[37,93,157,255],'background':[255,255,255]}, {'case':'opaque-nearest-checker','pixels':rgba,'background':[0,0,0]}, {'case':'alpha-nearest-checker','pixels':alpha,'background':[0,0,0]}, {'case':'alpha-texture-tint128','pixels':alpha,'tint_alpha':128,'background':[0,0,0]}, {'case':'partial-opaque-checker','pixels':rgba,'partial':[255,255,0,255],'background':[0,0,0]}, {'case':'partial-alpha-checker','pixels':rgba,'partial':[255,0,0,128],'background':[0,0,0]}]
for i,c in enumerate(cases):c['rect']=[20+180*(i%4),20+180*(i//4),150,150]
results={}
for phase in ['baseline','partial']:
 im=QImage(800,600,QImage.Format.Format_RGBA8888);im.fill(QColor(180,180,180));p=QPainter(im);p.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform,False)
 rows=[]
 for c in cases:
  x,y,w,h=c['rect'];rect=QRectF(x,y,w,h);p.fillRect(rect,QColor(*c['background']))
  if 'color'in c:p.fillRect(rect,QColor(*c['color']))
  else:
   pixels=[list(v)for v in c['pixels']]
   if phase=='partial'and'partial'in c:pixels[0]=c['partial']
   texture=QImage(bytes(sum(pixels,[])),2,2,8,QImage.Format.Format_RGBA8888).copy();p.save();p.setOpacity(c.get('tint_alpha',255)/255);p.drawImage(rect,texture);p.restore()
  points=[[x+37,y+37],[x+112,y+37],[x+37,y+112],[x+112,y+112]]
  values=[]
  for a,b in points:
   q=im.pixelColor(a,b);values.append([q.red(),q.green(),q.blue(),q.alpha()])
  rows.append({'case':c['case'],'points':points,'rgba':values})
 p.end();assert im.save(str(out/(phase+'.png')));results[phase]=rows
(out/'input.json').write_text(json.dumps({'cases':cases},indent=2)+'\n')
(out/'qpainter-results.json').write_text(json.dumps({'source':'Actual PySide6 QPainter source-over raster; no custom color blend calculation','phases':results},indent=2)+'\n')
