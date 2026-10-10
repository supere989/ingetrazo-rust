#!/usr/bin/env python3
"""Actual Qt DashLine per-edge alpha coverage; no port kernel reuse."""
import json,sys
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QPen,QColor
from PySide6.QtCore import Qt,QPointF
import numpy as np
rows=[]
cases=[("horizontal",[20.,40.],[140.,40.]),("horizontal-reversed",[140.,40.],[20.,40.]),("vertical",[40.,20.],[40.,140.]),("vertical-reversed",[40.,140.],[40.,20.]),("diagonal",[20.25,30.5],[170.75,140.125]),("diagonal-reversed",[170.75,140.125],[20.25,30.5]),("negative-slope",[30.125,180.5],[170.75,25.25]),("negative-slope-reversed",[170.75,25.25],[30.125,180.5]),("short",[33.25,33.75],[34.125,34.5]),("fractional",[31.125,49.75],[173.5,49.875])]
for name,a,b in cases:
 im=QImage(256,256,QImage.Format_ARGB32);im.fill(Qt.transparent)
 p=QPainter(im);p.setRenderHint(QPainter.Antialiasing,True);p.setPen(QPen(QColor(120,140,165),1,Qt.DashLine));p.drawLine(QPointF(*a),QPointF(*b));p.end()
 rgba=im.convertToFormat(QImage.Format_RGBA8888);v=np.frombuffer(rgba.constBits(),np.uint8).reshape(256,256,4)
 ys,xs=np.nonzero(v[:,:,3]);pixels=[[int(x),int(y),int(v[y,x,3])]for y,x in zip(ys,xs)]
 rows.append({"name":name,"a":a,"b":b,"pixels":pixels})
Path(sys.argv[1]).write_text(json.dumps({"qt_version":__import__("PySide6").__version__,"format":"ARGB32","cases":rows},indent=2)+"\n")
