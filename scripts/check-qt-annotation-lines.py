#!/usr/bin/env python3
import os,json,subprocess,sys,hashlib
from pathlib import Path
os.environ["QT_QPA_PLATFORM"]="offscreen"
from PySide6.QtGui import QImage,QPainter,QPen,QColor
from PySide6.QtCore import QPointF,QRect,qVersion
from PySide6.QtWidgets import QApplication
import numpy as np
assert qVersion().startswith("6.12.");app=QApplication([])
binary,out=Path(sys.argv[1]),Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
rows=[]
for points in [([20.,40.],[100.,40.]),([40.,20.],[40.,100.]),([20.,30.],[90.,85.]),([90.,30.],[20.,85.]),([20.,85.],[90.,30.]),([20.125,30.875],[100.625,42.25]),([-30.5,20.],[110.,90.])]:
 for width in [1.2,1.5,2.8,4.]:
  for phase in [0.,.25,.75]:rows.append(dict(a=[v+phase for v in points[0]],b=[v+phase for v in points[1]],width=width,clip=[0,0,128,128]))
rows.append(dict(a=[-50.,40.],b=[160.,40.],width=1.5,clip=[10,12,90,98]))
raw=json.dumps(rows);proc=subprocess.run([str(binary)],input=raw,text=True,capture_output=True,check=True,timeout=10);spans=json.loads(proc.stdout);assert len(spans)==len(rows);results=[]
for i,(row,span_rows)in enumerate(zip(rows,spans)):
 image=QImage(128,128,QImage.Format_RGBA8888);image.fill(QColor("white"));p=QPainter(image);p.setRenderHint(QPainter.Antialiasing);x,y,r,b=row["clip"];p.setClipRect(QRect(x,y,r-x,b-y));p.setPen(QPen(QColor("black"),row["width"]));p.drawLine(QPointF(*row["a"]),QPointF(*row["b"]));p.end();image=image.convertToFormat(QImage.Format_RGBA8888);actual=np.frombuffer(image.constBits(),np.uint8).reshape(128,image.bytesPerLine())[:,:512].reshape(128,128,4).copy();native=np.full((128,128,4),255,np.uint8)
 for x,y,n,c in span_rows:assert 0<=x<x+n<=128 and 0<=y<128;native[y,x:x+n,:3]=255-c
 count=int(np.any(actual!=native,axis=2).sum());results.append(dict(recipe=row,different_pixels=count))
 if count:image.save(str(out/(str(i)+"-source.png")));QImage(native.data,128,128,native.strides[0],QImage.Format_RGBA8888).save(str(out/(str(i)+"-native.png")))
report=dict(qt=qVersion(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),cases=len(rows),results=results,passed=all(r["different_pixels"]==0 for r in results));(out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print(report["cases"],"cases; failures",sum(r["different_pixels"]!=0 for r in results));assert report["passed"]
