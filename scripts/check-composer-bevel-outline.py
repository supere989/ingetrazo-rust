#!/usr/bin/env python3
import json,subprocess,sys,math
from pathlib import Path
from PySide6.QtGui import QPainterPath,QPainterPathStroker,QPen,QColor,QImage,QPainter
r=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(exist_ok=True,parents=True);rows=[]
for n in [3,4,7,24]:
 for reverse in [False,True]:
  points=[[64+40*math.cos(i*math.tau/n),64+35*math.sin(i*math.tau/n)] for i in range(n)]
  if reverse:points.reverse()
  inp=out/f'{n}-{reverse}.json';inp.write_text(json.dumps({'points':points,'radius':1.4}));dest=out/f'{n}-{reverse}-native.json';subprocess.run([str(r/'target/debug/examples/composer_bevel_outline_fixture'),str(inp),str(dest)],check=True)
  native=json.loads(dest.read_text());srcpath=QPainterPath();srcpath.moveTo(*points[0])
  for p in points[1:]:srcpath.lineTo(*p)
  srcpath.closeSubpath();stroker=QPainterPathStroker(QPen(QColor('black'),2.8));qp=stroker.createStroke(srcpath)
  qimage=QImage(128,128,QImage.Format.Format_ARGB32);qimage.fill(QColor('white'));p=QPainter(qimage);p.setRenderHint(QPainter.RenderHint.Antialiasing,True);p.fillPath(qp,QColor('black'));p.end()
  ni=out/f'{n}-{reverse}-contours.json';ni.write_text(json.dumps(native));spanfile=out/f'{n}-{reverse}-spans.json';subprocess.run([str(r/'target/debug/examples/qt_gray_protocol'),str(ni),str(spanfile)],check=True);im=QImage(128,128,QImage.Format.Format_ARGB32);im.fill(QColor('white'))
  for x,y,l,c in json.loads(spanfile.read_text()):
   for xx in range(x,x+l):
    if 0<=xx<128 and 0<=y<128:im.setPixelColor(xx,y,QColor(255-c,255-c,255-c))
  diff=sum(im.pixel(x,y)!=qimage.pixel(x,y) for y in range(128) for x in range(128));rows.append({'sides':n,'reverse':reverse,'different_pixels':diff});im.save(str(out/f'{n}-{reverse}-native.png'));qimage.save(str(out/f'{n}-{reverse}-qt.png'))
report={'passed':all(x['different_pixels']==0 for x in rows),'cases':rows,'scope':'Actual QPainterPathStroker BevelJoin plus Qt winding raster vs native convex beveloutline and graycells.'};(out/'verdict.json').write_text(json.dumps(report,indent=2));print(json.dumps(report));sys.exit(0 if report['passed'] else 1)
