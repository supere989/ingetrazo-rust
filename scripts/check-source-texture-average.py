#!/usr/bin/env python3
"""Actual Qt scaled4x4/pixelColor oracle; compare cold native output unchanged."""
import ast,json,sys,hashlib,subprocess
from pathlib import Path
from PySide6.QtGui import QImage,QColor
from PySide6.QtCore import Qt,qVersion
assert qVersion()=="6.12.0"
root=Path(__file__).resolve().parents[1]
assert subprocess.check_output(["git","-C",str(root/"ingetrazo"),"rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
rows=[]
for row in open(sys.argv[1]):
 w,h,a,p=row.split(' ',3);w=int(w);h=int(h);a=a=='true';got=ast.literal_eval(p)
 image=QImage(w,h,QImage.Format_ARGB32 if a else QImage.Format_RGB32)
 for y in range(h):
  for x in range(w):image.setPixelColor(x,y,QColor((x*37+y*17+1)%256,(x*13+y*61+7)%256,(x*91+y*3+19)%256,(x*43+y*29+51)%256 if a else 255))
 out=image.scaled(4,4,Qt.IgnoreAspectRatio,Qt.SmoothTransformation)
 expected=[list(out.pixelColor(x,y).getRgb())for y in range(4)for x in range(4)]
 rows.append({'w':w,'h':h,'alpha':a,'expected':expected,'native':got,'pass':expected==got})
required={(w,h,a)for w in [1,2,3,4,5,7,8,9,13,32]for h in [1,2,3,4,5,7,8,9,13,32]for a in [False,True]}
assert len(rows)==200 and {(r['w'],r['h'],r['alpha'])for r in rows}==required,'Missing or duplicated mandatory average case'
json.dump({'cases':rows,'qt_version':qVersion(),'app_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','binary_sha256':hashlib.sha256(Path(sys.argv[3]).read_bytes()).hexdigest(),'native_pixels_sha256':hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest(),'native_source':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()},open(sys.argv[2],'w'),indent=2)
bad=[(r['w'],r['h'],r['alpha'],sum(e!=g for e,g in zip(r['expected'],r['native'])))for r in rows if not r['pass']]
print(json.dumps({'total':len(rows),'failed':bad}));sys.exit(bool(bad))
