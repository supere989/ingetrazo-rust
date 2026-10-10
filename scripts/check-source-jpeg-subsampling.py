#!/usr/bin/env python3
"""Independent ImageMagick JPEG encoder, actual Qt decoder, explicit SOF sampling."""
import hashlib,json,subprocess,sys,time
from pathlib import Path
from PySide6.QtGui import QImage
from PySide6.QtCore import qVersion
assert qVersion()=="6.12.0"
binary=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir()
version=subprocess.check_output(["convert","-version"],text=True)
def sampling(data):
 pos=2
 while pos<len(data):
  assert data[pos]==255
  while data[pos]==255:pos+=1
  marker=data[pos];pos+=1
  size=int.from_bytes(data[pos:pos+2],"big");payload=data[pos+2:pos+size];pos+=size
  if marker in [192,194]:return marker,[p for p in payload[7::3]]
 raise AssertionError("missing SOF")
rows=[]
for w,h in [(1,1),(2,3),(3,2),(4,4),(7,9),(17,13),(32,33),(65,31)]:
 raw=bytes(channel for y in range(h) for x in range(w) for channel in [(x*37+y*17+1)%256,(x*13+y*61+7)%256,(x*91+y*3+19)%256])
 ppm=out/f"{w}x{h}.ppm";ppm.write_bytes(f"P6\n{w} {h}\n255\n".encode()+raw)
 for factors,expected in [("1x1,1x1,1x1",[17,17,17]),("2x1,1x1,1x1",[33,17,17]),("1x2,1x1,1x1",[18,17,17]),("2x2,1x1,1x1",[34,17,17])]:
  for progressive in [False,True]:
   name=f"{w}x{h}-{factors.split(',')[0]}-progressive{int(progressive)}";jpeg=out/(name+".jpg");native=out/(name+".rgba")
   command=["convert",str(ppm),"-sampling-factor",factors,"-quality","85","-interlace","Plane" if progressive else "None",str(jpeg)]
   subprocess.run(command,check=True,timeout=10,capture_output=True)
   encoded=jpeg.read_bytes();sof,factors_found=sampling(encoded);assert factors_found==expected and sof==(194 if progressive else 192)
   image=QImage.fromData(encoded).convertToFormat(QImage.Format_RGBA8888);assert not image.isNull();qt=bytes(image.constBits())
   started=time.monotonic();process=subprocess.run([str(binary),str(jpeg),str(native)],capture_output=True,text=True,timeout=10)
   pixels=native.read_bytes() if native.exists() else b""
   rows.append(dict(name=name,sampling=factors_found,sof=sof,encoder_command=command,encoded_sha256=hashlib.sha256(encoded).hexdigest(),qt_sha256=hashlib.sha256(qt).hexdigest(),native_sha256=hashlib.sha256(pixels).hexdigest(),elapsed_seconds=time.monotonic()-started,returncode=process.returncode,stderr=process.stderr,qt_bytes=len(qt),native_bytes=len(pixels),changed=sum(a!=b for a,b in zip(qt,pixels)),exact=qt==pixels))
report=dict(qt=qVersion(),encoder=version,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),cases=rows,passed=all(r["exact"] for r in rows))
(out/"results.json").write_text(json.dumps(report,indent=2));print([(r["name"],r["returncode"],r["changed"]) for r in rows if not r["exact"]]);assert len(rows)==64
if not report["passed"]:sys.exit(1)
