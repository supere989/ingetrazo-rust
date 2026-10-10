#!/usr/bin/env python3
"""Observe real Qt PDF serialization rather than copying its number oracle."""
import os,sys,json,re,zlib,hashlib
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPdfWriter,QPainter,QPainterPath,QPen
from PySide6.QtCore import qVersion,Qt,QLineF
app=QApplication([]);root=Path(sys.argv[1]);rows=[]
for index,value in enumerate([55.11536795523,-.1234567899,-1e-10,999.9999999999,0.000000012345,123456.789999999,1.0000000001,-1000.123456789]):
 path=root/("qt-number-"+str(index)+".pdf");writer=QPdfWriter(str(path));p=QPainter(writer);p.setRenderHint(QPainter.RenderHint.Antialiasing,True);pen=QPen();pen.setWidthF(1.);pen.setCapStyle(Qt.PenCapStyle.RoundCap);pen.setJoinStyle(Qt.PenJoinStyle.RoundJoin);p.setPen(pen);shape=QPainterPath();shape.moveTo(value,20.);shape.lineTo(10.,21.);p.drawLine(QLineF(value,20.,10.,21.));p.end();del writer
 data=path.read_bytes();texts=[]
 for raw in re.findall(rb"stream\r?\n(.*?)\r?\nendstream",data,re.S):
  try:raw=zlib.decompress(raw)
  except zlib.error:pass
  texts.append(raw)
 encoded=re.search(rb"(-?[0-9]+(?:\.[0-9]+)?) 20 m",b"\n".join(texts));assert encoded,(index,texts)
 rows.append({"value":value,"encoded":encoded[1].decode(),"pdf_sha256":hashlib.sha256(data).hexdigest()})
(root/"qt-number-observations.json").write_text(json.dumps({"qt_version":qVersion(),"rows":rows},indent=2));print("PASS actual Qt PDF numeral observations",len(rows))
