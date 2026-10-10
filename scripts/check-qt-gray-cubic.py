#!/usr/bin/env python3
"""Literal actual Qt cubic path elements replayed by bounded native integer subdivision."""
import json,subprocess,sys
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QPainterPath,QPainterPathStroker,QPen,QColor
from PySide6.QtCore import Qt,QRectF
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
rows=[]
for name,rect in [('ellipse',QRectF(10.25,9.75,102.5,87.25)),('negative-ellipse',QRectF(-10.25,-9.75,102.5,87.25)),('rounded',QRectF(10.25,9.75,102.5,87.25))]:
 p=QPainterPath()
 if name=='rounded':p.addRoundedRect(rect,13.25,11.5)
 else:p.addEllipse(rect)
 rows.append((name,p))
for name,a,b in [('round-horizontal',(15.25,32.5),(108.75,32.5)),('round-diagonal',(12.25,15.75),(109.5,101.25)),('round-reversed',(109.5,101.25),(12.25,15.75))]:
 p=QPainterPath();p.moveTo(*a);p.lineTo(*b);pen=QPen(QColor('black'),2.8,Qt.PenStyle.SolidLine,Qt.PenCapStyle.RoundCap,Qt.PenJoinStyle.BevelJoin);stroke=QPainterPathStroker(pen);rows.append((name,stroke.createStroke(p)))
results=[]
for name,path in rows:
 contours=[];i=0
 while i<path.elementCount():
  e=path.elementAt(i)
  if e.type==QPainterPath.ElementType.MoveToElement:contours.append({'start':[e.x,e.y],'segments':[]});i+=1
  elif e.type==QPainterPath.ElementType.LineToElement:contours[-1]['segments'].append({'line':[e.x,e.y]});i+=1
  else:
   c2=path.elementAt(i+1);to=path.elementAt(i+2);contours[-1]['segments'].append({'cubic':[[e.x,e.y],[c2.x,c2.y],[to.x,to.y]]});i+=3
 source=out/(name+'.json');source.write_text(json.dumps(contours));dest=out/(name+'-spans.json');subprocess.run([str(binary),str(source),str(dest)],check=True)
 im=QImage(128,128,QImage.Format.Format_ARGB32);im.fill(QColor('white'));p=QPainter(im);p.setRenderHint(QPainter.RenderHint.Antialiasing);p.setPen(Qt.PenStyle.NoPen);p.setBrush(QColor('black'));p.drawPath(path);p.end();im.save(str(out/(name+'-qt.png')))
 native=QImage(128,128,QImage.Format.Format_ARGB32);native.fill(QColor('white'))
 for x,y,length,c in json.loads(dest.read_text()):
  for xx in range(x,x+length):
   if 0<=xx<128 and 0<=y<128:native.setPixelColor(xx,y,QColor(255-c,255-c,255-c))
 native.save(str(out/(name+'-native.png')));diff=sum(im.pixel(x,y)!=native.pixel(x,y)for y in range(128)for x in range(128));results.append({'name':name,'different_pixels':diff,'passed':diff==0})
report={'passed':all(r['passed']for r in results),'cases':results};(out/'verdict.json').write_text(json.dumps(report,indent=2));print(json.dumps(report));assert report['passed']
