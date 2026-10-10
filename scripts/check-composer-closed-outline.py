#!/usr/bin/env python3
"""Held-out actual Qt cubic stroke outlines vs production source offset API."""
import json,subprocess,sys
from pathlib import Path
from PySide6.QtGui import QPainterPath,QPainterPathStroker,QPen,QColor,QPainter,QImage
from PySide6.QtCore import Qt,QRectF
root=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True);results=[]
rows=[('ellipse-flat','ellipse',[90,12],0,.5),('ellipse-tall','ellipse',[12,90],0,1.5),('ellipse-circle','ellipse',[60,60],0,4),('ellipse-small','ellipse',[8,6],0,.35),('rounded-flat','rounded',[90,12],4,.5),('rounded-tall','rounded',[12,90],3,1.5),('rounded-wide','rounded',[60,40],9,4),('rounded-capsule','rounded',[60,20],10,.35)]
for name,kind,size,radius,width in rows:
 data={'kind':kind,'size':size,'radius':radius,'width':width};src=out/(name+'.json');src.write_text(json.dumps(data));paths=out/(name+'-paths.json');result=subprocess.run([str(root/'target/debug/examples/composer_closed_outline_fixture'),str(src),str(paths)],capture_output=True,text=True)
 if result.returncode:results.append({'name':name,'passed':False,'error':result.stderr});continue
 # Translate both complete source outlines before raster; scale1.
 nativepaths=json.loads(paths.read_text())
 for p in nativepaths:
  p['start']=[x+10 for x in p['start']]
  for s in p['segments']:
   if 'line'in s:s['line']=[x+10 for x in s['line']]
   else:s['cubic']=[[x+10 for x in pt]for pt in s['cubic']]
 paths.write_text(json.dumps(nativepaths));spans=out/(name+'-spans.json');subprocess.run([str(root/'target/debug/examples/qt_gray_cubic_protocol'),str(paths),str(spans)],check=True)
 path=QPainterPath()
 if kind=='ellipse':path.addEllipse(QRectF(0,0,*size))
 else:path.addRoundedRect(QRectF(0,0,*size),radius,radius)
 pen=QPen(QColor('black'),width,Qt.PenStyle.SolidLine,Qt.PenCapStyle.SquareCap,Qt.PenJoinStyle.BevelJoin);stroke=QPainterPathStroker(pen).createStroke(path)
 im=QImage(128,128,QImage.Format.Format_ARGB32);im.fill(QColor('white'));p=QPainter(im);p.setRenderHint(QPainter.RenderHint.Antialiasing);p.translate(10,10);p.setPen(Qt.PenStyle.NoPen);p.setBrush(QColor('black'));p.drawPath(stroke);p.end();im.save(str(out/(name+'-qt.png')))
 native=QImage(128,128,QImage.Format.Format_ARGB32);native.fill(QColor('white'))
 for x,y,length,c in json.loads(spans.read_text()):
  for xx in range(x,x+length):
   if 0<=xx<128 and 0<=y<128:native.setPixelColor(xx,y,QColor(255-c,255-c,255-c))
 native.save(str(out/(name+'-native.png')));diff=sum(im.pixel(x,y)!=native.pixel(x,y)for y in range(128)for x in range(128));results.append({'name':name,'passed':diff==0,'different_pixels':diff})
assert [r['name']for r in results]==[r[0]for r in rows]
report={'passed':all(r['passed']for r in results),'cases':results};(out/'verdict.json').write_text(json.dumps(report,indent=2));print(json.dumps(report));assert report['passed']
