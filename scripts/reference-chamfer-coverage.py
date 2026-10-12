#!/usr/bin/env python3
"""One actual chamfer operation: public mask paints, prefix destination and invariance."""
import hashlib,json,os,subprocess,sys,traceback,zipfile
from pathlib import Path
repo=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
assert not(out/'original-chamfer.json').exists()
for k,v in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[k]=str(out/v)
os.environ['QT_QPA_PLATFORM']='offscreen';original=(repo/'ingetrazo').resolve();PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
sys.path.insert(0,str(original))
from PySide6.QtCore import Qt,qVersion
from PySide6.QtGui import QPainter,QImage,QColor,QPainterPath
from PySide6.QtWidgets import QApplication,QMainWindow
from views.viewport import Viewport
from views.composer import ComposerWindow
import views.composer as composer
from formats import igz
app=QApplication([]);assert qVersion()=='6.12.0',qVersion()
folder=repo/'docs/audits/sheet-symbols';m=json.loads((folder/'source-manifest.json').read_text());archive=folder/'source-records.zip';assert hashlib.sha256(archive.read_bytes()).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(archive)as z:
 for name in ['title-chamfer-double.igz','title-chamfer-double.png']:
  data=z.read(name);entry=next(v for v in m['entries']if v['path']==name);assert len(data)==entry['bytes']and hashlib.sha256(data).hexdigest()==entry['sha256'];(out/name).write_bytes(data)
host=QMainWindow();host.viewport=Viewport();host.setCentralWidget(host.viewport);igz.load_into(host.viewport.scene,out/'title-chamfer-double.igz');window=ComposerWindow(host)
errors=[]
def error(t,e,tb):
 row={'type':t.__name__,'message':str(e),'traceback':''.join(traceback.format_exception(t,e,tb))};errors.append(row);print(row['traceback'],file=sys.stderr);(out/'callback-errors.json').write_text(json.dumps(errors,indent=2))
sys.excepthook=error
def state():return {'composition':window.comp.to_dict(),'version':host.viewport.scene.version,'composer_undo':len(window.history._undo),'composer_redo':len(window.history._redo),'model_undo':len(host.viewport.history.undo_stack),'model_redo':len(host.viewport.history.redo_stack)}
def raw(image):
 image=image.convertToFormat(QImage.Format_RGBA8888);data=bytes(image.constBits());return b''.join(data[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*4]for y in range(image.height()))
def save(image,name):
 data=raw(image);assert image.save(str(out/(name+'.png')));(out/(name+'.rgba')).write_bytes(data)
 return {'file':name+'.png','rgba':name+'.rgba','size':[image.width(),image.height()],'rgba_sha256':hashlib.sha256(data).hexdigest()}
def number(enum):return enum.value if hasattr(enum,'value')else int(enum)
def path_data(path):return [{'type':number(path.elementAt(i).type),'xy':[path.elementAt(i).x,path.elementAt(i).y]}for i in range(path.elementCount())]
def transform_data(t):return [t.m11(),t.m12(),t.m13(),t.m21(),t.m22(),t.m23(),t.m31(),t.m32(),t.m33()]
before=state();window.export_image(str(out/'ordinary-before.png'),100)
ordinary=QImage(str(out/'ordinary-before.png'));frozen=QImage(str(out/'title-chamfer-double.png'));assert raw(ordinary)==raw(frozen),'ordinary source no longer equals frozen case'
stages=[];ActualPainter=QPainter
class Observer(ActualPainter):
 def drawPath(self,path):
  pen=self.pen()
  if pen.widthF()==window.comp.cajetin.border_mm and self.brush().style()==Qt.NoBrush and path.elementCount()>=8:
   assert len(stages)==0,'ambiguous heavy operation'
   device=self.device();assert isinstance(device,QImage)and device.format()==QImage.Format_RGB32
   assert pen.color().alpha()==255 and self.opacity()==1.0 and self.compositionMode()==QPainter.CompositionMode_SourceOver
   row={'path':path_data(path),'fill_rule':number(path.fillRule()),'pen':{'width':pen.widthF(),'rgba':list(pen.color().getRgb()),'cap':number(pen.capStyle()),'join':number(pen.joinStyle()),'miter':pen.miterLimit(),'cosmetic':pen.isCosmetic()},'world_transform':transform_data(self.worldTransform()),'combined_transform':transform_data(self.combinedTransform()),'view_transform_enabled':self.viewTransformEnabled(),'window':[self.window().x(),self.window().y(),self.window().width(),self.window().height()],'viewport':[self.viewport().x(),self.viewport().y(),self.viewport().width(),self.viewport().height()],'opacity':self.opacity(),'composition':number(self.compositionMode()),'hints':number(self.renderHints()),'has_clip':self.hasClipping(),'clip_path':path_data(self.clipPath())if self.hasClipping()else[],'device':{'format':number(device.format()),'dpm':[device.dotsPerMeterX(),device.dotsPerMeterY()],'pixel_ratio':device.devicePixelRatio()},'prefix':save(device.copy(),'prefix')}
   for name,color in [('transparent',QColor(0,0,0,0)),('white',QColor(255,255,255)),('black',QColor(0,0,0))]:
    image=QImage(device.size(),QImage.Format_ARGB32_Premultiplied if name=="transparent" else device.format());image.setDotsPerMeterX(device.dotsPerMeterX());image.setDotsPerMeterY(device.dotsPerMeterY());image.setDevicePixelRatio(device.devicePixelRatio());image.fill(color)
    q=ActualPainter(image);q.setWindow(self.window());q.setViewport(self.viewport());q.setViewTransformEnabled(self.viewTransformEnabled());q.setWorldTransform(self.worldTransform());q.setRenderHints(q.renderHints(),False);q.setRenderHints(self.renderHints());q.setOpacity(self.opacity());q.setCompositionMode(self.compositionMode());q.setPen(pen);q.setBrush(self.brush());q.setBackground(self.background());q.setBackgroundMode(self.backgroundMode())
    if self.hasClipping():q.setClipPath(self.clipPath())
    assert q.renderHints()==self.renderHints()and transform_data(q.combinedTransform())==transform_data(self.combinedTransform()),"probe painter state differs"
    q.drawPath(QPainterPath(path));q.end();row[name]=dict(save(image,'heavy-'+name),device_format=number(image.format()),scope='public alpha mask on ARGB32 premultiplied'if name=='transparent'else'original RGB32 destination composition')
   # Delegate the identical ordinary operation. Probe snapshots never replace
   # its geometry, state, clip or paint route.
   super().drawPath(path);row['after_heavy']=save(device.copy(),'after-heavy');stages.append(row)
   return
  super().drawPath(path)
composer.QPainter=Observer
try:window.export_image(str(out/'ordinary-observed.png'),100)
finally:composer.QPainter=ActualPainter
assert len(stages)==1 and not errors
observed=QImage(str(out/'ordinary-observed.png'));assert raw(observed)==raw(ordinary)==raw(frozen),'probe altered ordinary frame'
assert state()==before,'probe/export changed source model/history'
record={'source_pin':PIN,'qt_version':qVersion(),'case':'title-chamfer-double','dpi':100,'source_archive_sha256':m['archive_sha256'],'ordinary':save(observed,'ordinary'),'stages':stages,'model_history_unchanged':True,'ordinary_frozen_equal':True,'observer_ordinary_equal':True,'callback_errors':errors,'scope':'Public actual QPainter drawPath mask-paint observations, NOT private QSpan interception or unique-coverage inference from RGB. One retained case only; original source unchanged.'}
(out/'original-chamfer.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'cases':1,'heavy_operations':len(stages),'qt':qVersion()}));window.hide();host.hide()
