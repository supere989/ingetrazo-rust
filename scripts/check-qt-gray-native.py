#!/usr/bin/env python3
"""Actual Qt AA polygon oracle for bounded native gray line/cell geometry."""
import json,subprocess,sys
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QPainterPath,QColor
from PySide6.QtCore import Qt
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]);out.mkdir(exist_ok=True,parents=True)
rows=[('negative-clipped',[[[-.25,-.25],[70.75,-.25],[70.75,80.25],[-.25,80.25]]]),('rectangle',[[[10.25,10.25],[110,10.25],[110,80],[10.25,80]]]),('trapezoid',[[[10.25,2.5],[119.25,18.5],[61.5,99.25],[2.75,63.5]]]),('reversed',[[[2.75,63.5],[61.5,99.25],[119.25,18.5],[10.25,2.5]]]),('hole',[[[10,10],[115,10],[115,100],[10,100]],[[30.25,30.25],[30.25,80.75],[90.5,80.75],[90.5,30.25]]]),('bevel-outline',[[[10,8.6],[110,8.6],[111.4,10],[111.4,80],[110,81.4],[10,81.4],[8.6,80],[8.6,10]],[[11.4,11.4],[11.4,78.6],[108.6,78.6],[108.6,11.4]]])]
results=[]
for name,contours in rows:
 input=out/(name+'.json');input.write_text(json.dumps(contours));dest=out/(name+'-spans.json');subprocess.run([str(binary),str(input),str(dest)],check=True)
 im=QImage(128,128,QImage.Format.Format_ARGB32);im.fill(QColor('white'));p=QPainter(im);p.setRenderHint(QPainter.RenderHint.Antialiasing,True);p.setPen(QColor(0,0,0,0));p.setBrush(QColor('black'));path=QPainterPath();path.setFillRule(Qt.FillRule.WindingFill)
 for c in contours:
  path.moveTo(*c[0])
  for x,y in c[1:]:path.lineTo(x,y)
  path.closeSubpath()
 p.drawPath(path);p.end();im.save(str(out/(name+'-qt.png')))
 actual=QImage(128,128,QImage.Format.Format_ARGB32);actual.fill(QColor('white'));spans=json.loads(dest.read_text())
 for x,y,length,c in spans:
  for xx in range(x,x+length):
   if 0<=xx<128 and 0<=y<128:actual.setPixelColor(xx,y,QColor(255-c,255-c,255-c))
 actual.save(str(out/(name+'-native.png')));diff=sum(im.pixel(x,y)!=actual.pixel(x,y) for y in range(128) for x in range(128));assert diff==0,(name,diff);results.append({'name':name,'spans':len(spans),'different_pixels':diff})
report={'passed':True,'cases':results,'boundary':'Actual QPainter winding AA polygons vs pureRust26.6 line/cell/sweep runs. Not source curve flattening/stroking or GPU performance.'};(out/'verdict.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
