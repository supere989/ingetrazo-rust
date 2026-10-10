import json
from pathlib import Path
from PySide6.QtGui import QImage
a=Path("/tmp/integer-root-original"); b=Path("/tmp/integer-root-hardware-capture/shapes/capture"); names=[r["name"] for r in json.loads((a/"original-commands.json").read_text())]; assert len(names)==len(set(names))==9; cases=[]
for n in names:
 x=QImage(str(a/(n+".png"))).convertToFormat(QImage.Format.Format_RGBA8888); y=QImage(str(b/(n+".png"))).convertToFormat(QImage.Format.Format_RGBA8888); assert not x.isNull() and not y.isNull() and x.size()==y.size(); xx,yy=bytes(x.constBits()),bytes(y.constBits()); changed=sum(xx[i:i+4]!=yy[i:i+4] for i in range(0,len(xx),4)); cases.append(dict(name=n,different_pixels=changed))
report=dict(passed=all(r["different_pixels"]==0 for r in cases),cases=cases,scope="Root independently rebuilt frozen8bf2d40; actual owned RTX5090 whole image captures against retained original Qt; admission metadata under review"); print(json.dumps(report)); (b/"strict-root.json").write_text(json.dumps(report,indent=2)+"\n"); assert report["passed"]
