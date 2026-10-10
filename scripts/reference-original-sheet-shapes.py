#!/usr/bin/env python3
"""Actual original shape painter: record draw geometry while painting real QImages."""
import sys,json,dataclasses,os
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPainter,QImage
from core.composition import FormaItem
from views.composer import paint_forma_mm
app=QApplication([])
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
class Recorder(QPainter):
 def __init__(self,image):super().__init__(image);self.commands=[]
 def record(self,path):
  brush=self.brush();pen=self.pen();self.commands.append({"path":path,"stroke":[pen.color().name(),pen.widthF()] if pen.style().value else None,"fill":[brush.color().name(),brush.color().alphaF()] if brush.style().value else None})
 def drawRect(self,r):self.record({"rect":[r.width(),r.height()],"radius":0.});super().drawRect(r)
 def drawRoundedRect(self,r,rx,ry):self.record({"rect":[r.width(),r.height()],"radius":rx});super().drawRoundedRect(r,rx,ry)
 def drawEllipse(self,r):self.record({"ellipse":[r.width(),r.height()]});super().drawEllipse(r)
 def drawPolygon(self,points):self.record({"polygon":[[p.x(),p.y()]for p in points]});super().drawPolygon(points)
 def drawLine(self,a,b):self.record({"line":[[a.x(),a.y()],[b.x(),b.y()]]});super().drawLine(a,b)
recipes=[];results=[]
for name,kwargs in [("rect",{}),("rounded",{"radius_mm":8.,"fill":True}),("ellipse",{"kind":"elipse","fill":True}),("polygon-clamped",{"kind":"poligono","sides":30,"fill":True}),("line-inverted",{"kind":"linea","invert":True}),("arrow",{"kind":"flecha"}),("terrain-ticks",{"kind":"terreno"}),("terrain-hatch-inverted",{"kind":"terreno","ground":"hatch","invert":True}),("terrain-band",{"kind":"terreno","ground":"band"})]:
 shape=FormaItem(**kwargs);recipes.append(dataclasses.asdict(shape));image=QImage(600,500,QImage.Format.Format_ARGB32);image.fill(0xffffffff);p=Recorder(image);p.setRenderHint(QPainter.RenderHint.Antialiasing,os.environ.get("INGETRAZO_SHAPE_ANTIALIAS")=="1");zoom=float(os.environ.get("INGETRAZO_SHAPE_ZOOM","8"));ppp=float(os.environ.get("INGETRAZO_SHAPE_PPP","1"));ox=float(os.environ.get("INGETRAZO_SHAPE_ORIGIN_X","50"));oy=float(os.environ.get("INGETRAZO_SHAPE_ORIGIN_Y","50"));p.translate(ox*ppp,oy*ppp);p.scale(zoom*ppp,zoom*ppp);paint_forma_mm(p,shape);p.end();assert image.save(str(out/(name+".png")))
 results.append({"name":name,"commands":p.commands})
(out/"recipes.json").write_text(json.dumps(recipes,indent=2)+"\n");(out/"original-commands.json").write_text(json.dumps(results,indent=2)+"\n");print("PASS actual original shape painter",len(results))

(out/"transform.json").write_text(json.dumps({"zoom":zoom,"ppp":ppp,"origin":[ox,oy]}))
