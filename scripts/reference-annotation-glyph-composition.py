#!/usr/bin/env python3
"""Actual Qt RGBA8888 glyph masks/colored destinations; no fitted palette."""
import os,sys,json,hashlib
from pathlib import Path
os.environ["QT_QPA_PLATFORM"]="offscreen"
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPainter,QImage,QColor,QFont
from PySide6.QtCore import QPointF,qVersion
import numpy as np
assert qVersion().startswith("6.12.")
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def pixels(im):return np.frombuffer(im.constBits(),np.uint8).reshape(400,640,4).copy()
def mul(value,alpha):
 t=value*alpha+32768;return(t+(t>>16))>>16
font=QFont("Sans Serif");font.setPointSizeF(11);font.setBold(True)
rows=[]
for name,bg in [("glyph-light",[240,243,246]),("glyph-dark",[17,43,91])]:
 image=QImage(640,400,QImage.Format_RGBA8888);image.fill(QColor(*bg));expected=pixels(image).astype(np.uint64)
 ops=[dict(kind="polygon",pen=[0,0,0,0],width=0,brush=bg+[255],points=[[-1000,-1000],[4000,-1000],[-1000,4000]])]
 painter=QPainter(image);painter.setRenderHint(QPainter.TextAntialiasing);painter.setFont(font)
 for row,rgb in enumerate([[1,7,19],[37,93,157],[255,255,255]]):
  for col,alpha in enumerate([0,1,127,128,230,255]):
   position=[25+col*100,60+row*100];rgba=rgb+[alpha]
   painter.setPen(QColor(*rgba));painter.drawText(QPointF(*position),"Mg")
   ops.append(dict(kind="text",pen=rgba,width=1,font=["Sans Serif",11,True],position=position,text="Mg"))
   mask=QImage(640,400,QImage.Format_RGBA8888);mask.fill(QColor(0,0,0,0));mp=QPainter(mask);mp.setRenderHint(QPainter.TextAntialiasing);mp.setFont(font);mp.setPen(QColor(0,0,0));mp.drawText(QPointF(*position),"Mg");mp.end();coverage=pixels(mask)[:,:,3].astype(np.uint64)*257
   dst=expected[:,:,:3]*257;src=mul(np.array(rgb,np.uint64)*257,alpha*257)
   if alpha==255:value=mul(src,coverage[:,:,None])+mul(dst,65535-coverage[:,:,None])
   else:
    colored=src+mul(dst,65535-alpha*257);value=mul(colored,coverage[:,:,None])+mul(dst,65535-coverage[:,:,None])
   expected[:,:,:3]=(value+128-((value+128)>>8))>>8
 painter.end();actual=pixels(image);count=int(np.any(actual!=expected,axis=2).sum());assert count==0,(name,count)
 image.save(str(out/(name+".png")));rows.append(dict(name=name,operations=ops,pixels_sha256=hashlib.sha256(actual.tobytes()).hexdigest(),image_sha256=hashlib.sha256((out/(name+".png")).read_bytes()).hexdigest(),qrga64_different_pixels=count))
(out/"original.json").write_text(json.dumps(dict(source_pin="6be29fe437a7cd992d4e079f71720821b25216b9",qt=qVersion(),cases=rows,scope="Actual Qt6.12 RGBA8888 generic glyph composition on opaque colored targets; other target formats/platforms remain required"),indent=2)+"\n")
print("36 actual colored glyph controls exact QRgba64 composition")
