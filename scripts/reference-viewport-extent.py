"""Actual pinned Qt viewport resize/DPR observations; no rendering acceptance."""
import json,sys,time,subprocess
from pathlib import Path
source=Path(sys.argv[1]);out=Path(sys.argv[2])
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path.insert(0,str(source))
from PySide6.QtCore import Qt
from PySide6.QtGui import QSurfaceFormat
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
class Observed(Viewport):
 def resizeGL(self,w,h):
  super().resizeGL(w,h)
  self.resize_records.append(dict(callback=[w,h],logical=[self.width(),self.height()],dpr=self.devicePixelRatioF(),aspect=self.camera.aspect,scene_fbo=list(self._fbo_size)))
vp=Observed();vp.resize_records=[];vp.setWindowFlags(vp.windowFlags()|Qt.WindowTransparentForInput|Qt.WindowDoesNotAcceptFocus)
rows=[]
try:
 for width,height in [(942,610),(943,611),(801,601)]:
  vp.setFixedSize(width,height);vp.show();vp.update();deadline=time.monotonic()+1
  while time.monotonic()<deadline:app.processEvents();time.sleep(.01)
  image=vp.grabFramebuffer();assert not image.isNull();assert vp.resize_records
  rows.append(dict(logical=[vp.width(),vp.height()],dpr=vp.devicePixelRatioF(),fb_size=list(vp._fb_size()),grab=[image.width(),image.height()],aspect=vp.camera.aspect,resize_records=vp.resize_records.copy()))
  vp.resize_records.clear()
finally:
 vp.close();vp.deleteLater();app.processEvents()
out.write_text(json.dumps(dict(scope='Actual original resize policy only; no image parity claim',rows=rows),indent=2)+'\n')
