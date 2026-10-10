#!/usr/bin/env python3
"""Valid PDF negative controls for exact alignment and source wrap clipping."""
import os,sys,re,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtCore import QSize
from PySide6.QtGui import QImage,QPainter
app=QApplication([]);original,native=map(Path,sys.argv[1:3]);rows=[]
def render(path):
 doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;assert doc.pageCount()==1;size=doc.pagePointSize(0);image=doc.render(0,QSize(round(size.width()*300/72),round(size.height()*300/72)));assert not image.isNull();out=QImage(image.size(),QImage.Format.Format_RGBA8888);out.fill(0xffffffff);p=QPainter(out);p.drawImage(0,0,image);p.end();return bytes(out.constBits())
for name in ["center","wrap"]:
 path=native/(name+".native.pdf");raw=path.read_bytes()
 if name=="center":
  m=re.search(rb"(-?\d+\.\d+) (-?\d+\.\d+) Td ",raw);assert m
  value=format(float(m[1])+1/64,"."+str(len(m[1].split(b".")[1]))+"f").encode();assert len(value)<=len(m[1]);value=value.ljust(len(m[1]),b" ");changed=raw[:m.start(1)]+value+raw[m.end(1):]
 else:
  if os.environ.get("INGETRAZO_TEXT_PARAGRAPHS")=="1":
   # Typed paragraph writer restores clip per independent font/line draw.
   # Remove every occurrence so this remains a missing-rectangle-clip counterexample.
   assert raw.count(b"W* n")==5
   changed=raw.replace(b"W* n",b"n   ")
  else:
   assert raw.count(b"W* n")==1;changed=raw.replace(b"W* n",b"n   ",1)
 assert len(changed)==len(raw);mutated=native/(name+".valid-mutated.pdf");mutated.write_bytes(changed)
 a=render(original/(name+".original.pdf"));b=render(mutated);assert len(a)==len(b);count=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));assert count>0,name+" mutation must be rejected"
 rows.append(dict(case=name,changed_pixels=count,parse_valid=True,same_size=True,passed=True))
report=dict(passed=True,cases=rows,scope="Actual reader, whole 300dpi pages; 1/64px alignment and removed rectangle clip controls")
(native/"layout-negatives.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report))
