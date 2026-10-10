#!/usr/bin/env python3
import hashlib,json,subprocess,sys,zlib
from pathlib import Path
from PySide6.QtGui import QImage,QColor
from PySide6.QtCore import Qt
binary=Path(sys.argv[1]);fixture=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir()
source=fixture.read_bytes()
cases={"truncated":source[:64],"wrong_magic":b"not-a-jpeg","invalid_metadata":bytes.fromhex("ffd8ffe10001ffd9")}
large=bytearray(source);pos=2
while pos<len(large):
 while large[pos]==255:pos+=1
 marker=large[pos];pos+=1;size=int.from_bytes(large[pos:pos+2],"big")
 if marker in [192,194]:large[pos+3:pos+7]=b"\xff\xff\xff\xff";break
 pos+=size
cases["oversized_sof"]=bytes(large)
results=[]
for name,encoded in cases.items():
 inputpath=out/(name+".jpg");output=out/(name+".rgba");inputpath.write_bytes(encoded);output.write_bytes(b"old-ready-output")
 p=subprocess.run([str(binary),str(inputpath),str(output)],capture_output=True,text=True,timeout=5)
 results.append(dict(name=name,returncode=p.returncode,stderr=p.stderr,previous_output_preserved=output.read_bytes()==b"old-ready-output",refused=p.returncode!=0))
output=out/"budget.rgba";output.write_bytes(b"old-ready-output")
p=subprocess.run([str(binary),str(fixture),str(output),"1024"],capture_output=True,text=True,timeout=5)
results.append(dict(name="actual_low_work_budget",returncode=p.returncode,stderr=p.stderr,previous_output_preserved=output.read_bytes()==b"old-ready-output",refused=p.returncode!=0))
for alpha in [0,1,127,128,254,255]:
 image=QImage(7,5,QImage.Format_ARGB32);image.fill(QColor(1,7,19,alpha));path=out/f"alpha{alpha}.png";assert image.save(str(path),"PNG")
 q=QImage(str(path)).convertToFormat(QImage.Format_RGBA8888);expected=bytes(q.constBits());output=out/f"alpha{alpha}.rgba"
 p=subprocess.run([str(binary),str(path),str(output)],capture_output=True,text=True,timeout=5)
 results.append(dict(name=f"PNG_alpha{alpha}",returncode=p.returncode,exact=output.exists() and output.read_bytes()==expected,expected_sha256=hashlib.sha256(expected).hexdigest()))
assert len(results)==11
passed=all(r.get("exact",r.get("refused",False) and r.get("previous_output_preserved",False)) for r in results)
(out/"results.json").write_text(json.dumps(dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),cases=results,passed=passed),indent=2));print(passed)
if not passed:sys.exit(1)
