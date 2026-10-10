import json
from pathlib import Path
from PySide6.QtGui import QImage
import sys
a=Path(sys.argv[1]); b=Path(sys.argv[2]); names=[r["name"] for r in json.loads((a/"original-commands.json").read_text())]; expected=["rect","rounded","ellipse","polygon-clamped","line-inverted","arrow","terrain-ticks","terrain-hatch-inverted","terrain-band"]; assert names==expected; cases=[]
for n in names:
 x=QImage(str(a/(n+".png"))).convertToFormat(QImage.Format.Format_RGBA8888); y=QImage(str(b/(n+".png"))).convertToFormat(QImage.Format.Format_RGBA8888); assert not x.isNull() and not y.isNull() and x.size()==y.size(); xx,yy=bytes(x.constBits()),bytes(y.constBits()); changed=sum(xx[i:i+4]!=yy[i:i+4] for i in range(0,len(xx),4)); cases.append(dict(name=n,different_pixels=changed))
changed_one=bytearray(yy); changed_one[0]^=1; assert sum(xx[i:i+4]!=changed_one[i:i+4] for i in range(0,len(xx),4))==1
assert names[:-1]!=expected and list(reversed(names))!=expected and names[:-1]+[names[0]]!=expected
report=dict(negative_controls=dict(one_pixel_rejected=True,missing_duplicate_reordered_rejected=True),passed=all(r["different_pixels"]==0 for r in cases),cases=cases,scope="Root independently rebuilt21bf58c; actual owned RTX5090 whole600x500 thin-zoom2 images against fresh frozen-original Qt6.12 controls; no full AA/platform claim"); print(json.dumps(report)); (b/"strict-root.json").write_text(json.dumps(report,indent=2)+"\n"); assert report["passed"]
