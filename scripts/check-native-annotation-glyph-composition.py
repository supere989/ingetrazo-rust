#!/usr/bin/env python3
"""Strict actual native glyph operator and literal old-byte negative."""
import os,sys,json,subprocess,hashlib
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage
root=Path(__file__).resolve().parents[1];binary,source,out=map(Path,sys.argv[1:4]);out.mkdir(parents=True,exist_ok=True)
original=json.loads((source/"original.json").read_text());assert original["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
assert [c["name"]for c in original["cases"]]==["glyph-light","glyph-dark"]
def pixels(path,native=False):
 im=QImage(str(path));assert not im.isNull(),"Unreadable image"
 assert(im.width(),im.height())==((800,600)if native else(640,400)),"Wrong extent"
 im=im.convertToFormat(QImage.Format_RGBA8888);a=np.frombuffer(im.constBits(),np.uint8).reshape(im.height(),im.bytesPerLine())[:,:im.width()*4].reshape(im.height(),im.width(),4).copy()
 if native:assert np.all(a[400:]==255)and np.all(a[:400,640:]==255),"Extra exterior ink"
 return a[:400,:640]
def compare(a,b):assert a.shape==b.shape;return int(np.any(a!=b,axis=2).sum())
rows=[]
try:
 for name in ["glyph-light","glyph-dark","dim-mm-axis-byte-negative"]:
  negative=name.endswith("byte-negative");case=out/name
  oracle=root/"docs/audits/native-annotation-source/original.json" if negative else source/"original.json"
  reference=root/"docs/audits/native-annotation-source/dim-mm-axis.png"if negative else source/(name+".png")
  env=dict(os.environ,INGETRAZO_CAD2D_INTEGER_BLEND="1",INGETRAZO_ANNOTATION_ORACLE=str(oracle),INGETRAZO_ANNOTATION_CASE="dim-mm-axis"if negative else name)
  if negative:env["INGETRAZO_ANNOTATION_GLYPH_BYTE_CONTROL"]="1"
  result=subprocess.run(["python3",str(root/"scripts/check-native-vulkan-startup.py"),str(binary),str(case)],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=30)
  case.mkdir(exist_ok=True);(case/"driver.log").write_text(result.stdout);assert result.returncode==0,"Native lifecycle failed"
  capture=case/"resize-800x600.png";s,n=pixels(reference),pixels(capture,True);count=compare(s,n)
  # The negative changes only immutable glyph operator, preserving all geometry and colors.
  assert count==9 if negative else count==0,(name,count)
  altered=n.copy();altered[0,0,0]^=1;assert compare(n,altered)==1,"Pixel mutation negative ineffective"
  rows.append(dict(name=name,different_pixels=count,source_sha256=hashlib.sha256(reference.read_bytes()).hexdigest(),native_sha256=hashlib.sha256(capture.read_bytes()).hexdigest()))
finally:
 (out/"result.json").write_text(json.dumps(dict(source_sha=sys.argv[4],binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),oracle_sha256=hashlib.sha256((source/"original.json").read_bytes()).hexdigest(),rows=rows,passed=len(rows)==3),indent=2)+"\n")
print("36 colored glyph controls exact; old-byte operator fails9pixels; pixel mutation rejected")
