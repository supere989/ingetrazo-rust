#!/usr/bin/env python3
"""Actual original production paper vector painter at 8 pixels per mm."""
import sys,json,os
from pathlib import Path
sys.path.insert(0,os.environ.get("INGETRAZO_ORIGINAL_ROOT","ingetrazo"));os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage,QPainter,QColor
from core.composition import MarcoVista,frame_page_projector
from core.camera import OrbitCamera
from views.composer import paint_frame_mm
app=QApplication([]);reference=json.loads(Path(sys.argv[1]).read_text());camera=OrbitCamera();camera.target=__import__("PySide6.QtGui",fromlist=["QVector3D"]).QVector3D(*reference["camera"]["target"])
for k,v in reference["camera"].items():
 if k not in ["target","up"]:setattr(camera,k,v)
width=float(sys.argv[3]);frame=MarcoVista(view_key="std:front",scale_n=100.,w_mm=100.,h_mm=100.,style="vectorial");frame.pen_edge_mm=width;frame.pen_profile_mm=width;frame.pen_cut_mm=width;frame.hidden_lines=True;frame.cut_fill="none";frame.border=False;frame.grid_m=0
drawing=reference["drawings"]["true"];page=frame_page_projector(frame,camera);segs=[]
for x0,y0,x1,y1 in drawing["segs"]:
 a,b=page(x0,y0),page(x1,y1);segs.append([*a,*b])
image=QImage(800,800,QImage.Format_RGBA8888);image.fill(QColor("white"));painter=QPainter(image);painter.setRenderHint(QPainter.Antialiasing);painter.scale(8,8);paint_frame_mm(painter,frame,None,hlr=segs,kinds=drawing["kinds"]);painter.end();image.save(sys.argv[2])
