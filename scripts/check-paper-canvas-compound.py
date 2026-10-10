#!/usr/bin/env python3
"""Execute operative original paper-stack statements; compare native cold spans."""
import os,sys,json,subprocess,hashlib,textwrap
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtCore import Qt,QRectF,qVersion
from PySide6.QtGui import QColor,QPainter,QImage,QPen,QBrush
from PySide6.QtWidgets import QApplication,QGraphicsScene
assert qVersion()=="6.12.0";app=QApplication([])
root=Path(__file__).resolve().parents[1];original=(root/"ingetrazo").resolve();assert subprocess.check_output(["git","-C",str(original),"rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
source=(original/"views/composer.py").read_text();start=source.index('        shadow = self.canvas.addRect(2.0, 2.0, pw, ph, QPen(Qt.NoPen),');end=source.index('        margin.setZValue(-100001)',start)+len('        margin.setZValue(-100001)');code=textwrap.dedent(source[start:end])
recipes=[{"name":"fractional-paper","page":[50.,30.],"margin":2.,"density":2.3,"phase":[.37,.23]},{"name":"thin-paper","page":[50.,30.],"margin":3.,"density":1.5,"phase":[.23,.67]},{"name":"integer-paper","page":[50.,30.],"margin":2.,"density":2.,"phase":[0.,0.]}]
recipes+=[{'name':'wide-page','page':[30.,20.],'margin':2.,'density':4.,'phase':[.37,.23]},{'name':'wide-guide','page':[20.,12.],'margin':1.5,'density':6.25,'phase':[.23,.67]}]
binary=Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/paper_compound_span_fixture";rows=json.loads(subprocess.check_output([str(binary)],input=json.dumps(recipes),text=True));assert [r['name']for r in rows]==[r['name']for r in recipes]
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);report=[]
def mul(x,a):v=x*a+128;return(v+(v>>8))>>8
for recipe,row in zip(recipes,rows):
 assert 'error'not in row,row
 scene=QGraphicsScene();owner=SimpleNamespace(canvas=scene,comp=SimpleNamespace(margin_mm=recipe['margin']),_add_guide_items=lambda:None)
 exec(code,globals(),dict(self=owner,pw=recipe['page'][0],ph=recipe['page'][1]))
 image=QImage(160,110,QImage.Format_ARGB32_Premultiplied);image.fill(QColor(210,210,210));p=QPainter(image);p.setRenderHint(QPainter.Antialiasing);p.translate(*recipe['phase']);p.scale(recipe['density'],recipe['density']);scene.render(p,QRectF(-10,-10,80,55),QRectF(-10,-10,80,55),Qt.IgnoreAspectRatio);p.end()
 native=QImage(160,110,QImage.Format_ARGB32_Premultiplied);native.fill(QColor(210,210,210))
 for layer in row['layers']:
  r,g,b,a=layer['premul'];premul=[r,g,b]
  for x,y,length,coverage in layer['spans']:
   sa=mul(a,coverage);rgb=[mul(c,coverage)for c in premul]
   for px in range(x,x+length):
    if 0<=px<160 and 0<=y<110:
     old=native.pixelColor(px,y);out_rgb=[min(255,c+mul(d,255-sa))for c,d in zip(rgb,[old.red(),old.green(),old.blue()])];native.setPixelColor(px,y,QColor(*out_rgb))
 changed=[]
 for y in range(110):
  for x in range(160):
   if image.pixel(x,y)!=native.pixel(x,y):changed.append([x,y,image.pixelColor(x,y).getRgb(),native.pixelColor(x,y).getRgb()])
 image.save(str(out/(row['name']+'.source.png')));native.save(str(out/(row['name']+'.native.png')));report.append({'name':row['name'],'changed':len(changed),'first':changed[:12]})
result={'source_statements_sha256':hashlib.sha256(code.encode()).hexdigest(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'recipes':recipes,'cases':report,'scope':'Actual original QGraphicsScene operative page stack vs cold native spans; production GPU capture remains mandatory'};(out/'report.json').write_text(json.dumps(result,indent=2)+'\n');print(report);raise SystemExit(0 if all(r['changed']==0 for r in report)else 1)
