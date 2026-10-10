#!/usr/bin/env python3
"""Strict complete-page actual original/native vector PDF reader comparison."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
app=QApplication([]);root=Path(sys.argv[1]);dpi=int(sys.argv[2]);rows=[]
for path in sorted(root.glob("*.original.pdf")):
 name=path.name.removesuffix(".original.pdf");docs=[]
 for role in ["original","native"]:
  doc=QPdfDocument();assert doc.load(str(root/(name+"."+role+".pdf")))==QPdfDocument.Error.None_;assert doc.pageCount()==2;docs.append(doc)
 for page in range(2):
  assert docs[0].pagePointSize(page)==docs[1].pagePointSize(page);images=[]
  for role,doc in zip(["original","native"],docs):
   size=doc.pagePointSize(page);raw=doc.render(page,QSize(round(size.width()*dpi/72),round(size.height()*dpi/72)));assert not raw.isNull();image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);p=QPainter(image);p.drawImage(0,0,raw);p.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888);image.save(str(root/(name+"."+role+"."+str(page)+"."+str(dpi)+".png")));images.append(bytes(image.constBits()))
  a,b=images;changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));rows.append({"case":name,"hidden":bool(page),"different_pixels":changed,"passed":changed==0})
assert len(rows)==26;report={"passed":all(row["passed"]for row in rows),"dpi":dpi,"reader":"QtPdf/PDFium","strict_rgba":True,"cases":rows,"scope":"Actual original ComposerWindow whole-page PDF versus native cached persistent HLR/direct vector writer, mixed shape/view item order, source caps/joins/hidden dashes/pen widths and clip. No font/raster style/UI parity claim."};(root/("reader-"+str(dpi)+".json")).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report));sys.exit(0 if report["passed"]else 1)
