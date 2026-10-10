#!/usr/bin/env python3
"""Observe actual original frame painter drawLine grid commands; no replacement geometry."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
import numpy as np
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPainter,QImage
from views.composer import paint_frame_mm
from core.composition import MarcoVista
app=QApplication([])
class Observe(QPainter):
 def drawLine(self,a,b):
  self.lines.append([[a.x(),a.y()],[b.x(),b.y()]])
  return super().drawLine(a,b)
rows=[]
for grid,perspective in [(0.,False),(-1.,False),(.199999,False),(.2,False),(.7,False),(.7,True)]:
 image=QImage(120,120,QImage.Format.Format_ARGB32_Premultiplied);image.fill(0xffffffff)
 p=Observe(image);p.lines=[]
 f=MarcoVista(w_mm=100,h_mm=100,scale_n=100,grid_m=grid,perspective=perspective,style="vectorial",cut_fill="none",annotations=False,border=False,show_title=False)
 paint_frame_mm(p,f,None,hlr=np.array([[1.,1.,2.,2.]]))
 lines=p.lines;p.end();rows.append({"grid_m":grid,"perspective":perspective,"lines":lines})
Path(sys.argv[1]).write_text(json.dumps({"source":"views.composer.paint_frame_mm","cases":rows},indent=2)+"\n")
print("PASS actual original grid observer six cutoff/perspective controls")
