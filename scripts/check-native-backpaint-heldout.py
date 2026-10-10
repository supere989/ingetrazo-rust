import json,sys
from pathlib import Path
from PySide6.QtGui import QImage
def load(p):
 im=QImage(str(p));assert not im.isNull();return im
def pixel(im,p):
 c=im.pixelColor(*p);return (c.red(),c.green(),c.blue(),c.alpha())
a=Path(sys.argv[1]);b=Path(sys.argv[2]);original=json.loads((a/"original-alpha-results.json").read_text())["cases"];native=json.loads((b/"native-alpha-results.json").read_text())["cases"]
expected=["back-slanted-reversed","back-holed-reversed","back-reflected-group","back-group-opaque-gate"]
assert [x["case"]for x in original]==expected==[x["case"]for x in native]
failures=[]
for o,n in zip(original,native):
 assert "NVIDIA"in o["gl_info"]["renderer"] and not o["gl_info"]["software"]
 im1=load(a/(o["case"]+".png"));im2=load(b/(n["case"]+".png"));assert (im1.width(),im1.height())==(im2.width(),im2.height())==(800,600)
 assert list(pixel(im1,(400,300)))==o["center_rgba"] and list(pixel(im2,(400,300)))==n["center_rgba"]
 for point in [(400,300),(500,300),(400,400),(300,300)]:
  if pixel(im1,point)!=pixel(im2,point):failures.append({"case":o["case"],"point":point,"source":pixel(im1,point),"native":pixel(im2,point)})
assert pixel(load(a/"back-holed-reversed.png"),(400,300))!=pixel(load(a/"back-holed-reversed.png"),(500,300)),"Hole test must include actual painted ring"
print(json.dumps({"passed":not failures,"literal_pixel_failures":failures,"source_reference":"viewport.py8035-8061 tp32 mirror triangle swap"},indent=2))
if "--retain-open"in sys.argv:
 assert failures and {x["case"]for x in failures}=={"back-reflected-group"}
else:assert not failures,failures
