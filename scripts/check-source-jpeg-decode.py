#!/usr/bin/env python3
"""Actual Qt JPEG decode controls including progressive, grayscale and odd dimensions."""
import hashlib,json,subprocess,sys,time
from pathlib import Path
from PySide6.QtGui import QImage,QImageWriter
from PySide6.QtCore import QBuffer,QByteArray,QIODevice,qVersion
assert qVersion()=="6.12.0"
source_pin=subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip()
assert source_pin=="6be29fe437a7cd992d4e079f71720821b25216b9"
binary=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir()
rows=[]
for w,h in [(1,1),(2,3),(3,2),(4,4),(7,9),(17,13),(32,33),(65,31)]:
 for gray in [False,True]:
  for progressive in [False,True]:
   image=QImage(w,h,QImage.Format_Grayscale8 if gray else QImage.Format_RGB32)
   for y in range(h):
    for x in range(w):
     r=(x*37+y*17+1)%256;g=r if gray else (x*13+y*61+7)%256;b=r if gray else (x*91+y*3+19)%256
     image.setPixelColor(x,y,__import__("PySide6.QtGui",fromlist=["QColor"]).QColor(r,g,b))
   array=QByteArray();buffer=QBuffer(array);assert buffer.open(QIODevice.WriteOnly)
   writer=QImageWriter(buffer,b"jpeg");writer.setQuality(85);writer.setProgressiveScanWrite(progressive);assert writer.write(image)
   encoded=bytes(array);name=f"{w}x{h}-gray{int(gray)}-progressive{int(progressive)}";inputpath=out/(name+".jpg");rawpath=out/(name+".rgba");inputpath.write_bytes(encoded)
   decoded=QImage.fromData(encoded).convertToFormat(QImage.Format_RGBA8888);expected=bytes(decoded.constBits())
   started=time.monotonic()
   process=subprocess.run([str(binary),str(inputpath),str(rawpath)],capture_output=True,text=True,timeout=10)
   actual=rawpath.read_bytes() if rawpath.exists() else b""
   rows.append(dict(name=name,jpeg_sha256=hashlib.sha256(encoded).hexdigest(),expected_sha256=hashlib.sha256(expected).hexdigest(),native_sha256=hashlib.sha256(actual).hexdigest(),returncode=process.returncode,elapsed_seconds=time.monotonic()-started,stdout=process.stdout,stderr=process.stderr,expected_bytes=len(expected),native_bytes=len(actual),changed=sum(a!=b for a,b in zip(expected,actual)),exact=actual==expected))
report=dict(qt=qVersion(),source_pin=source_pin,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),cases=rows,passed=all(r["exact"] for r in rows))
(out/"results.json").write_text(json.dumps(report,indent=2))
print([(r["name"],r["returncode"],r["changed"]) for r in rows if not r["exact"]])
assert len(rows)==32 and len({r["name"] for r in rows})==32
if not report["passed"]:sys.exit(1)
