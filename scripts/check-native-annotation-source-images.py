#!/usr/bin/env python3
"""Strict whole-image source operation corpus; native viewport acquisition is separate."""
import argparse,subprocess,os,json,hashlib
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage
p=argparse.ArgumentParser();p.add_argument("binary");p.add_argument("output");p.add_argument("--source",required=True);a=p.parse_args();assert len(a.source)==40
root=Path(__file__).resolve().parents[1];out=Path(a.output);out.mkdir(parents=True,exist_ok=True)
oracle=root/"docs/audits/native-annotation-source/original.json";data=json.loads(oracle.read_text());assert data["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
names=["arrow","tick-selected","none","partial-occlusion","hidden","leader-right","leader-left-selected","leader-anchor-unprojectable","leader-hidden","leader-empty","dim-hover","dim-mm-axis","dim-feet-fractional","leader-hover","leader-splitlines"]
assert [r["name"]for r in data["cases"]]==names,"Mandatory source inventory changed"
def pixels(path,native=False):
 im=QImage(str(path));assert not im.isNull()
 if native:
  assert im.width()==800 and im.height()==600,"Native canvas extent changed"
  full=im.convertToFormat(QImage.Format_RGBA8888);array=np.frombuffer(full.constBits(),np.uint8).reshape(600,full.bytesPerLine())[:,:3200].reshape(600,800,4)
  assert np.all(array[400:]==255)and np.all(array[:400,640:]==255),"Extra ink outside the declared source canvas"
 else:assert im.width()==640 and im.height()==400,"Original canvas extent changed"
 im=im.copy(0,0,640,400).convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(im.constBits(),np.uint8).reshape(400,im.bytesPerLine())[:,:2560].reshape(400,640,4).copy()
rows=[]
try:
 for name in names:
  case=out/name;env=dict(os.environ,INGETRAZO_CAD2D_INTEGER_BLEND="1",INGETRAZO_ANNOTATION_CASE=name,INGETRAZO_ANNOTATION_ORACLE=str(oracle))
  result=subprocess.run(["python3",str(root/"scripts/check-native-vulkan-startup.py"),a.binary,str(case)],env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=30)
  case.mkdir(exist_ok=True);(case/"driver.log").write_text(result.stdout)
  assert result.returncode==0,f"Native case lifecycle failed: {name}"
  source=root/"docs/audits/native-annotation-source"/(name+".png");native=case/"resize-800x600.png";s,n=pixels(source),pixels(native,True);diff=np.any(s!=n,axis=2);coords=np.argwhere(diff)
  row=dict(name=name,source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),native_sha256=hashlib.sha256(native.read_bytes()).hexdigest(),different_pixels=len(coords),first=[dict(x=int(c[1]),y=int(c[0]),source=s[tuple(c)].tolist(),native=n[tuple(c)].tolist())for c in coords[:20]])
  rows.append(row);print(name,len(coords),flush=True)
finally:
 report=dict(source_sha=a.source,binary_sha256=hashlib.sha256(Path(a.binary).read_bytes()).hexdigest(),oracle_sha256=hashlib.sha256(oracle.read_bytes()).hexdigest(),rows=rows,passed=len(rows)==15 and all(r["different_pixels"]==0 for r in rows),scope="Actual native Vulkan draw of recorded original operation sequence; real projection/depth/font-platform/application source parity remains required")
 (out/"result.json").write_text(json.dumps(report,indent=2)+"\n")
assert report["passed"],"Strict source annotation image parity remains FAIL"
