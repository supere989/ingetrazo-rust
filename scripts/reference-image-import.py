#!/usr/bin/env python3
"""One bounded original Image chooser/pointer/VCB/history capture.
Initial geometry, chooser paths and camera are declared fixtures. Original methods unchanged.
"""
import ast,atexit,hashlib,json,os,shutil,struct,subprocess,sys,traceback
from pathlib import Path
out=Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=False);repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
os.environ['QT_QPA_PLATFORM']='offscreen';os.environ['XDG_CONFIG_HOME']=str(out/'config');os.environ['XDG_DATA_HOME']=str(out/'data');os.environ['XDG_CACHE_HOME']=str(out/'cache');sys.path.insert(0,str(original))
from PySide6.QtCore import Qt,QPoint,QPointF,QEvent,QTimer,QBuffer,QByteArray,QIODevice
from PySide6.QtGui import QImage,QImageReader,QMouseEvent,QMatrix4x4 as M,QVector3D as V
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtTest import QTest
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.image_plane import image_aspect
from formats import igz
from views.main_window import MainWindow
from views.filedialogs import file_dialogs
import core.texture
app=QApplication([]);states=[];cases=[];headers=[];errors=[];dialogs=[]
sys.excepthook=lambda t,e,tb:errors.append(''.join(traceback.format_exception(t,e,tb)))
def partial():
 if not(out/'original-image-import.json').exists():(out/'original-image-import.partial.json').write_text(json.dumps({'cases':cases,'states':states,'headers':headers,'errors':errors,'dialogs':dialogs},indent=2)+'\n')
atexit.register(partial)
m=ast.parse((repo/'scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in m.body if isinstance(n,ast.FunctionDef)and n.name in ['vec','mesh_state','shape']],type_ignores=[]),'borrowed-geometry-observer','exec'))
hashbytes=lambda b:hashlib.sha256(b).hexdigest()
def rgba(path):
 q=QImage(str(path));assert not q.isNull(),str(path);q=q.convertToFormat(QImage.Format_RGBA8888);b=bytes(q.constBits());return b''.join(b[y*q.bytesPerLine():y*q.bytesPerLine()+q.width()*4]for y in range(q.height())),[q.width(),q.height()]
def resource(path):
 b,size=rgba(path);return {'encoded_sha256':hashbytes(Path(path).read_bytes()),'rgba_sha256':hashbytes(b),'size':size}
def observe(s):
 g=shape(s);images=[]
 for p in s.image_planes:
  d=p.to_dict();d['texture']['path']=resource(p.path);images.append(d)
 selection=[]
 for e in s.selection:
  if e in s.image_planes:selection.append({'kind':'image','index':s.image_planes.index(e)})
  elif e in s.groups:selection.append({'kind':'group','index':s.groups.index(e)})
  else:raise AssertionError('Undeclared selected entity')
 return {'geometry':{k:g[k]for k in ['active','loose','groups']},'datum':None if s.georef is None else s.georef.to_dict(),'images':images,'layers':[l.to_dict()for l in s.layers],'units':g['units'],'selection':sorted(selection,key=lambda p:(p['kind'],p['index']))}
def camera(c):return {'target':vec(c.target),'distance':c.distance,'yaw':c.yaw,'pitch':c.pitch,'projection':'perspective'if c.perspective else'parallel','fov':c.fov_deg}
def tool_state(tool):
 image=getattr(tool,'name','')=='Image'
 return {'name':tool.name,'armed':bool(getattr(tool,'armed',False)),'aspect':tool.aspect if image else None,'resource':resource(tool.path)if image and tool.path else None,'start':vec(tool.start_point)if image and tool.start_point is not None else None,'hover':vec(tool.hover_point)if image and tool.hover_point is not None else None,'work_plane':[vec(p)for p in tool.work_plane]if image and tool.work_plane is not None else None,'free_aspect':tool._free_aspect if image else False,'preview_lines':[[vec(a),vec(b)]for a,b in tool.rubber_band_lines()]if image else[],'value_label':([tool.value_label()[0],vec(tool.value_label()[1])]if image and tool.value_label()is not None else None)}
assets=out/'assets';assets.mkdir();q=QImage(24,12,QImage.Format_RGBA8888)
for y in range(12):
 for x in range(24):q.setPixelColor(x,y,__import__('PySide6.QtGui',fromlist=['QColor']).QColor((x*11)%256,(y*23)%256,(x*7+y*13)%256,0 if x<3 else 128 if x<6 else 255))
for filename,fmt in [('scan.png','PNG'),('scan.webp','WEBP'),('scan.bmp','BMP'),('base.jpg','JPEG')]:assert q.save(str(assets/filename),fmt,92)
# EXIF orientation is fixture metadata; header aspect is observed independently.
b=(assets/'base.jpg').read_bytes();tiff=b'II'+struct.pack('<HI',42,8)+struct.pack('<H',1)+struct.pack('<HHI',274,3,1)+struct.pack('<H',6)+b'\0\0'+b'\0'*4;exif=b'Exif\0\0'+tiff;(assets/'scan-exif.jpg').write_bytes(b[:2]+b'\xff\xe1'+struct.pack('>H',len(exif)+2)+exif+b[2:])
# Small unassociated-alpha16 TIFF with orientation3; actual Qt decode is the oracle.
w,h=24,12;pix=bytearray()
for y in range(h):
 for x in range(w):pix+=struct.pack('<4H',x*2711%65536,y*5957%65536,(x*4001+y*3251)%65536,0 if x<3 else 32768 if x<6 else 65535)
entries=[(256,4,[w]),(257,4,[h]),(258,3,[16]*4),(259,3,[1]),(262,3,[2]),(273,4,[0]),(274,3,[3]),(277,3,[4]),(278,4,[h]),(279,4,[len(pix)]),(284,3,[1]),(338,3,[2])];entries.sort();start=8+2+12*len(entries)+4;bitspos=start;datapos=start+8;blob=bytearray(b'II*\0'+struct.pack('<I',8)+struct.pack('<H',len(entries)))
for tag,typ,v in entries:
 raw=struct.pack('<'+('H'if typ==3 else'I')*len(v),*([datapos]if tag==273 else v));blob+=struct.pack('<HHI',tag,typ,len(v));blob+=struct.pack('<I',bitspos)if len(raw)>4 else raw.ljust(4,b'\0')
blob+=b'\0'*4+struct.pack('<4H',16,16,16,16)+pix;(assets/'scan16.tif').write_bytes(blob);(assets/'invalid.png').write_bytes(b'this is not an image\n')
for p in sorted(assets.iterdir()):
 before=QImageReader.allocationLimit();aspect,pw,ph=image_aspect(p);assert QImageReader.allocationLimit()==before
 row={'name':p.name,'file':'assets/'+p.name,'aspect':aspect,'size':[pw,ph],'readable':pw>0 and ph>0,'encoded_sha256':hashbytes(p.read_bytes()),'decoded':None}
 if row['readable']:
  raw,size=rgba(p);dest='assets/'+p.name+'.rgba';(out/dest).write_bytes(raw);row['decoded']={'rgba':dest,'size':size,'sha256':hashbytes(raw)}
 headers.append(row)
def action(phase,op,**kw):return dict(phase=phase,op=op,**kw)
def put(file='scan.png',phase='armed'):return action(phase,'import',asset='assets/'+file,filename=file)
def pointer(phase,op,p,shift=False):return action(phase,op,world=p,shift=shift)
def standard(file='scan.png',typed=None):
 rows=[put(file),pointer('hover-before','move',[-4,-2,0]),pointer('first-press','press',[-4,-2,0]),pointer('first-release','release',[-4,-2,0]),pointer('hover-sized','move',[-1,2,0])]
 rows+=([action('typed-commit','vcb',text=typed)]if typed else[pointer('second-press','press',[-1,2,0]),pointer('second-release','release',[-1,2,0])]);rows+=[action('undo','undo'),action('redo','redo')];return rows
roster=[('png-drag-transparency',standard(),[]),('nested-width-cm',standard(typed='250cm'),[0]),('jpeg-exif-pair',standard('scan-exif.jpg','10cm;20cm'),[]),('webp-reuse-names',standard('scan.webp','2m')+[put('scan.webp','armed-repeat'),pointer('repeat-hover','move',[-4,-2,0]),pointer('repeat-first','press',[-4,-2,0]),pointer('repeat-release','release',[-4,-2,0]),pointer('repeat-size','move',[-1,2,0]),action('repeat-commit','vcb',text='2m'),action('undo-repeat','undo'),action('redo-repeat','redo'),action('armed-renamed','import',asset='assets/scan.webp',filename='renamed.webp'),pointer('rename-hover','move',[-4,-2,0]),pointer('rename-first','press',[-4,-2,0]),pointer('rename-release','release',[-4,-2,0]),pointer('rename-size','move',[-1,2,0]),action('rename-commit','vcb',text='2m')],[]),('bmp-free-aspect',standard('scan.bmp')[:-4]+[pointer('hover-free','move',[-1,2,0],True),pointer('second-press','press',[-1,2,0],True),pointer('second-release','release',[-1,2,0],True),action('undo','undo'),action('redo','redo')],[]),('tiff16-orientation',standard('scan16.tif','1.5m'),[]),('cancel-invalid-escape',[action('chooser-cancel','import',asset=None,filename=None),put('invalid.png','invalid-header'),put(),pointer('hover-before','move',[-4,-2,0]),pointer('first-press','press',[-4,-2,0]),pointer('first-release','release',[-4,-2,0]),pointer('hover-sized','move',[-1,2,0]),action('escape','escape'),action('escape-again','escape'),pointer('retry-hover','move',[-4,-2,0]),pointer('retry-first','press',[-4,-2,0]),pointer('retry-release','release',[-4,-2,0]),pointer('retry-size','move',[-1,2,0]),action('retry-commit','vcb',text='1m'),action('switch-line','tool',name='Line')],[]),('oblique-face-width',standard(typed='3m'),[])]
for name,actions,edit_path in roster:
 win=MainWindow();vp=win.viewport;s=vp.scene;vp.setFixedSize(800,600);win.show();app.processEvents();s.units={'length':'cm','precision':3}
 vp.camera.perspective=False;vp.camera.set_view('top');vp.camera.target=V(-2,0,0);vp.camera.distance=20;
 if name=='jpeg-exif-pair':
  for a in actions:
   if a.get('world')==[-1,2,0]:a['world']=[-6,-4,0]
 mesh=Mesh();points=[]
 for a in actions:
  if 'world'in a and a['world']not in points:points.append(a['world'])
 if name=='oblique-face-width':points=[[-4,-2,0],[-1,2,3]];mesh.add_face([V(-6,-4,-1.5),V(2,-4,-1.5),V(-6,6,6)])
 for p in points:mesh.add_edge(V(*p),V(p[0]+.05,p[1]+.03,p[2]))
 if edit_path:
  g=Group(mesh,'Context');g.xform=M();g.xform.translate(0,0,0);s.groups=[g]
 else:s.mesh=mesh;s.loose_mesh=mesh
 initial=name+'-input.igz';igz.save_scene(s,out/initial)
 if edit_path:vp.begin_group_edit(s.groups[0])
 # Oblique fixture uses its declared second endpoint; all actual contexts remain observed.
 if name=='oblique-face-width':
  actions.insert(1,action('armed-external-removed','remove-external'))
  for a in actions:
   if a.get('world')==[-1,2,0]:a['world']=[-1,2,3]
 core.texture.texture_cache_root=lambda:out/'cache'/name;initial_camera=camera(vp.camera);external=out/'external'/name;external.mkdir(parents=True);imported=[];held=False
 def record(phase):states.append({'case':name,'phase':phase,'scene':observe(s),'tool':tool_state(vp.active_tool),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'camera':camera(vp.camera)})
 record('before')
 for a in actions:
  op=a['op'];previous=len(s.image_planes);history=len(vp.history.undo_stack)
  if op=='import':
   p=None
   if a['asset'] is not None:p=external/a['filename'];shutil.copyfile(out/a['asset'],p)
   file_dialogs.getOpenFileName=lambda *args,**kw:('','')if p is None else(str(p),'')
   completed=[False];expired=[False]
   def dismiss():
    widget=app.activeModalWidget()
    if not isinstance(widget,QMessageBox):return
    try:
     assert not expired[0],'Image message deadline expired'
     assert a['phase']=='invalid-header','Unexpected actual source message dialog'
     dialogs.append({'case':name,'phase':a['phase'],'text':widget.text()});QTest.keyClick(widget,Qt.Key_Return)
    except BaseException:sys.excepthook(*sys.exc_info())
    finally:widget.reject();completed[0]=True
   def deadline(completed=completed,expired=expired):
    if not completed[0]:
     expired[0]=True;errors.append('Image import/message fixture deadline expired')
   timer=QTimer();timer.timeout.connect(dismiss);timer.start(10);QTimer.singleShot(5000,deadline)
   try:win._on_import_image()
   finally:completed[0]=True;timer.stop()
   if p is not None and a['filename']!='invalid.png':assert vp.active_tool.name=='Image'and vp.active_tool.armed;imported.append(vp.active_tool.path)
  elif op in ['move','press','release']:
   projected=vp._world_to_pixel(V(*a['world']));assert projected is not None;pixel=QPoint(round(projected[0]),round(projected[1]));a['pixel']=[pixel.x(),pixel.y()];a['viewport_size']=[vp.width(),vp.height()];mod=Qt.ShiftModifier if a.get('shift')else Qt.NoModifier
   if op=='move':ev=QMouseEvent(QEvent.MouseMove,QPointF(pixel),QPointF(pixel),Qt.NoButton,Qt.LeftButton if held else Qt.NoButton,mod);QApplication.sendEvent(vp,ev)
   elif op=='press':QTest.mousePress(vp,Qt.LeftButton,mod,pixel);held=True
   else:QTest.mouseRelease(vp,Qt.LeftButton,mod,pixel);held=False
  elif op=='vcb':QTest.keyClicks(vp,a['text']);QTest.keyClick(vp,Qt.Key_Return);assert len(s.image_planes)==previous+1 and len(vp.history.undo_stack)==history+1
  elif op=='undo':assert vp.history.undo()
  elif op=='redo':assert vp.history.redo()
  elif op=='escape':QTest.keyClick(vp,Qt.Key_Escape);assert vp.active_tool.name=='Image'and vp.active_tool.armed and vp.active_tool.start_point is None
  elif op=='tool':win._activate_tool('line')
  elif op=='remove-external':
   for p in external.iterdir():p.unlink()
  else:raise AssertionError(op)
  app.processEvents();record(a['phase']);assert not errors,errors
  if name=='oblique-face-width' and a['phase']=='first-press':
   assert vp.active_tool.work_plane is not None,'Actual face plane not acquired'
   normal=vp.active_tool.work_plane[1].normalized();assert abs(normal.z())<.999,'Oblique cohort fell back to ground'
  if name=='oblique-face-width' and op=='vcb':
   assert all(abs(V.dotProduct(edge,normal))<=1e-5*max(1.,edge.length())for edge in [s.image_planes[-1].u,s.image_planes[-1].v]),'Committed plane edges left acquired face'
  if op=='press' and len(s.image_planes)>previous:assert len(s.image_planes)==previous+1 and len(vp.history.undo_stack)==history+1
  if op=='import':assert len(s.image_planes)==previous and len(vp.history.undo_stack)==history
  (out/'original-image-import.partial.json').write_text(json.dumps({'cases':cases,'states':states,'headers':headers,'errors':errors,'dialogs':dialogs},indent=2,allow_nan=False)+'\n')
 # Source resource survives deleting every original chooser file before saving.
 for p in external.iterdir():p.unlink()
 record('external-removed')
 assert all(Path(p.path).is_file()for p in s.image_planes),'cached resource lost after chooser file removal'
 if edit_path:vp.end_group_edit();record('finish')
 saved=name+'-saved.igz';igz.save_scene(s,out/saved);loaded=Scene();igz.load_into(loaded,out/saved);assert observe(loaded)['images']==observe(s)['images'];record('save')
 if name=='webp-reuse-names':assert imported[0]==imported[1] and imported[0]!=imported[2]
 assert s.image_planes,'case did not create an actual image'
 cases.append({'name':name,'input':initial,'saved':saved,'edit_path':edit_path,'initial_camera':initial_camera,'viewport_size':[800,600],'actions':actions,'phases':[r['phase']for r in states if r['case']==name],'cache_reuse':{'same_name_equal':len(imported)<2 or imported[0]==imported[1],'renamed_distinct':len(imported)<3 or imported[0]!=imported[2]}})
 (out/'completed-cases.json').write_text(json.dumps({'cases':cases,'states':states},indent=2,allow_nan=False)+'\n')
 win.hide();win.deleteLater();app.processEvents();assert not errors,errors
(out/'original-image-import.json').write_text(json.dumps({'source_pin':PIN,'cases':cases,'states':states,'headers':headers,'dialogs':dialogs,'callback_errors':errors,'scope':'Original chooser/header, actual Qt pointer/VCB, independent history and original save/load; no rendered frame pixel claim'},indent=2,allow_nan=False)+'\n');print(json.dumps({'cases':len(cases),'states':len(states),'headers':len(headers)}))
