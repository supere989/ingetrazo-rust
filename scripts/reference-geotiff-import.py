#!/usr/bin/env python3
"""Original decoder and actual orthophoto App action; fixture inputs declared.
No source edits/native runs. Capture only after independent producer prereview.
"""
import atexit,ast,hashlib,json,os,shutil,struct,subprocess,sys,traceback,zlib
from pathlib import Path
out=Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=False)
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
os.environ['QT_QPA_PLATFORM']='offscreen';os.environ['XDG_CONFIG_HOME']=str(out/'config');os.environ['XDG_DATA_HOME']=str(out/'data');sys.path.insert(0,str(original))
import numpy as np
from PySide6.QtCore import QTimer,Qt,QBuffer,QByteArray,QIODevice
from PySide6.QtGui import QImage,QMatrix4x4 as M,QVector3D as V
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtTest import QTest
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer
from georef.datum import SceneDatum
from georef.geotiff import read_info,read_rgba,GeoTiffError
from tests.geotiff_fixture import write_geotiff,_packbits,_lzw_encode
from formats import igz
from views.main_window import MainWindow
from views.filedialogs import file_dialogs
import core.texture
app=QApplication([]);errors=[]
def preserve_partial():
 if not (out/'original-geotiff.json').exists():
  data={k:globals().get(k,[])for k in ['cases','states','decodes','resources','defects','dialogs','errors']};(out/'original-geotiff.partial.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
atexit.register(preserve_partial)
sys.excepthook=lambda typ,value,tb:errors.append(''.join(traceback.format_exception(typ,value,tb)))
m=ast.parse((repo/'scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[v for v in m.body if isinstance(v,ast.FunctionDef)and v.name in ['vec','mesh_state','shape']],type_ignores=[]),'retained-model-observer','exec'))
def picture(h=17,w=23,seed=1):
 a=np.random.default_rng(seed).integers(0,256,(h,w,4),dtype=np.uint8);a[:,:,3]=255;a[:3,:4,3]=0;return a

def supplemental(path,px,*,big=False,bo='<',planar=1,bits=8,photo=2,compression=1,predictor=1,rows=3,geographic=False,point=False,citation=False):
 """Typed supplemental TIFF fixture writer, never expected pixel generation."""
 h,w,samples=px.shape;dtype=np.dtype(bo+'u2')if bits==16 else np.dtype('u1');blocks=[]
 for ch in (range(samples)if planar==2 else[None]):
  for y in range(0,h,rows):
   a=px[y:y+rows]if ch is None else px[y:y+rows,:,ch:ch+1]
   if predictor==2:a=np.diff(a.astype(np.int64),axis=1,prepend=0)
   raw=a.astype(dtype).tobytes();blocks.append(zlib.compress(raw)if compression in(8,32946)else _packbits(raw)if compression==32773 else _lzw_encode(raw)if compression==5 else raw)
 entries=[(256,4,[w]),(257,4,[h]),(258,3,[bits]*samples),(259,3,[compression]),(262,3,[photo]),(277,3,[samples]),(278,4,[rows]),(284,3,[planar]),(317,3,[predictor]),(273,16 if big else 4,[0]*len(blocks)),(279,16 if big else 4,[len(b)for b in blocks]),(339,3,[1]*samples)]
 if samples==4:entries.append((338,3,[2]))
 if geographic:
  entries.append((34264,12,[.0001,.00002,0,-75,.00001,-.0001,0,-15,0,0,1,0,0,0,0,1]));keys=[1024,0,1,2,1025,0,1,2 if point else 1,2048,0,1,4326]
 else:
  entries.extend([(33550,12,[.25,.5,0]),(33922,12,[0,0,0,720106.4,8289766.2,0])]);keys=[1024,0,1,1,1025,0,1,1,3072,0,1,32767 if citation else 32718]
  if citation:
   text=b'WGS 84 / UTM zone 18S|\0';entries.append((34737,2,text));keys.extend([3073,34737,len(text),0])
 entries.append((34735,3,[1,1,0,len(keys)//4]+keys));entries.sort();inline=8 if big else 4;start=16 if big else 8;entrysize=20 if big else 12;countsize=8 if big else 2;nextsize=8 if big else 4
 def packed(typ,v):return bytes(v)if typ==2 else struct.pack(bo+{3:'H',4:'I',12:'d',16:'Q'}[typ]*len(v),*v)
 cursor=start+countsize+len(entries)*entrysize+nextsize;pos={}
 for tag,typ,v in entries:
  raw=packed(typ,v)
  if len(raw)>inline:pos[tag]=cursor;cursor+=len(raw)+(len(raw)&1)
 offsets=[]
 for b in blocks:offsets.append(cursor);cursor+=len(b)+(len(b)&1)
 entries=[(t,ty,offsets if t==273 else v)for t,ty,v in entries];data=bytearray((b'II'if bo=='<'else b'MM')+struct.pack(bo+'H',43 if big else 42));data+=struct.pack(bo+'HHQ',8,0,start)if big else struct.pack(bo+'I',start);data+=struct.pack(bo+('Q'if big else'H'),len(entries));blobs=[]
 for tag,typ,v in entries:
  raw=packed(typ,v);data+=struct.pack(bo+('HHQ'if big else'HHI'),tag,typ,len(v))
  if len(raw)>inline:data+=struct.pack(bo+('Q'if big else'I'),pos[tag]);blobs.append(raw)
  else:data+=raw.ljust(inline,b'\0')
 data+=bytes(nextsize)
 for b in blobs+blocks:data+=b+bytes(len(b)&1)
 path.write_bytes(data)
def qrgba(q):
 assert not q.isNull();q=q.convertToFormat(QImage.Format_RGBA8888);raw=bytes(q.constBits());return b''.join(raw[y*q.bytesPerLine():y*q.bytesPerLine()+q.width()*4]for y in range(q.height())),[q.width(),q.height()]
def plane_state(p):
 d=p.to_dict();path=Path(p.path);raw,size=qrgba(QImage(str(path)));d['texture']['path']={'encoded_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'rgba_sha256':hashlib.sha256(raw).hexdigest(),'size':size};return d
def observe(s):
 g=shape(s);sel=[]
 for e in s.selection:
  if e in s.image_planes:sel.append({'kind':'image','index':s.image_planes.index(e)})
  elif e in s.groups:sel.append({'kind':'group','index':s.groups.index(e)})
  else:raise AssertionError('Undeclared selected geometry owner')
 return {'datum':None if s.georef is None else s.georef.to_dict(),'images':[plane_state(p)for p in s.image_planes],'layers':[v.to_dict()for v in s.layers],'geometry':{k:g[k]for k in ['active','loose','groups']},'selection':sorted(sel,key=lambda v:(v['kind'],v['index'])),'units':g['units']}
def camera(c):return {'target':vec(c.target),'distance':c.distance,'yaw':c.yaw,'pitch':c.pitch,'projection':'perspective'if c.perspective else'parallel','fov':c.fov_deg}
recipes=[]
def add(name,a,**kw):
 p=out/(name+'.tif');write_geotiff(p,a,**kw);recipes.append((name,p,40 if 'reduced'in name else 8192))
a=picture();add('strip-raw-rgba8',a);add('strip-deflate-predictor8',a,compression=8,predictor=2);add('strip-packbits-grayalpha8',a,samples=2,compression=32773);add('strip-lzw-rgb8',a,samples=3,compression=5);add('tile-raw-partial',a,tile=16);add('tile-deflate-reduced',picture(70,100,3),tile=32,compression=8,predictor=2);add('tile-lzw-reduced',picture(70,100,3),tile=32,compression=5,predictor=2)
for name,px,kw,maxpx in [('old-deflate-odd-carry',picture(13,19),dict(compression=32946),8),('bigtiff-be-planar16',a.astype(np.uint16)*257,dict(big=True,bo='>',planar=2,bits=16,predictor=2,compression=8),8192),('classic-planar8',a[:,:,:3],dict(planar=2,compression=8),8192),('whitezero16',a[:,:,:1].astype(np.uint16)*257,dict(bits=16,photo=0),8192),('rgba16-be-predictor',a.astype(np.uint16)*257,dict(bo='>',bits=16,predictor=2),8192),('geographic-affine-point',a,dict(geographic=True,point=True),8192),('citation-utm',a,dict(citation=True),8192)]:
 p=out/(name+'.tif');supplemental(p,px,**kw);recipes.append((name,p,maxpx))
p=out/'jpeg-tables-cmyk.tif';shutil.copyfile(original/'tests/data/orto_jpeg_tile.tif',p);recipes.append(('jpeg-tables-cmyk',p,8192))
rgb=picture(32,32);q=QImage(rgb.tobytes(),32,32,128,QImage.Format_RGBA8888).copy();ba=QByteArray();buf=QBuffer(ba);buf.open(QIODevice.WriteOnly);assert q.save(buf,'JPEG',92);buf.close();p=out/'jpeg-rgb.tif';write_geotiff(p,rgb,samples=3,tile=32,compression=7,jpeg_blobs=[bytes(ba)]);recipes.append(('jpeg-rgb',p,8192))
decodes=[]
for name,p,maxpx in recipes:
 raw,factor=read_rgba(read_info(p),max_px=maxpx);f=name+'-decoded.rgba';(out/f).write_bytes(raw.tobytes());decodes.append({'case':name,'input':p.name,'max_px':maxpx,'factor':factor,'size':[raw.shape[1],raw.shape[0]],'rgba':f})
decoder_controls={}
try:read_rgba(read_info(out/'strip-raw-rgba8.tif'),progress=lambda fraction:False)
except GeoTiffError as e:decoder_controls['api_cancel']={'refused':True,'message':str(e)}
else:raise AssertionError('Original decoder ignored cancellation')
supplemental(out/'gui-reduced.tif',picture(67,2050,7),compression=8,predictor=2)
p=out/'decode-failure.tif';write_geotiff(p,a,compression=8);raw=bytearray(p.read_bytes());info=read_info(p);raw[info.offsets[0]]=0;p.write_bytes(raw)
cases=[];states=[];resources=[];defects=[];dialogs=[]
for name in ['fresh-datum','existing-rotated-datum','gui-reduced','chooser-cancel','decode-failure-datum','nested-global-image']:
 win=MainWindow();vp=win.viewport;s=vp.scene;mesh=Mesh();mesh.add_face([V(0,0,0),V(2,0,0),V(0,3,0)]);s.groups=[Group(mesh,'Existing')];s.units={'length':'cm','precision':3}
 if name in ['existing-rotated-datum','nested-global-image']:s.georef=SceneDatum(-15.46,-72.95,37.5,33);s.layers.append(Layer('Images'))
 initial=name+'-input.igz';igz.save_scene(s,out/initial)
 if name=='nested-global-image':vp.begin_group_edit(s.groups[0])
 tiff='jpeg-tables-cmyk.tif'if name=='fresh-datum'else'gui-reduced.tif'if name=='gui-reduced'else'decode-failure.tif'if name=='decode-failure-datum'else'tile-deflate-reduced.tif';ceiling=1024 if name=='gui-reduced'else 8192;vp.max_texture_size=lambda:ceiling
 core.texture.texture_cache_root=lambda:out/'cache'/name;file_dialogs.getOpenFileName=lambda *a,**k:('', '')if name=='chooser-cancel'else(str(out/tiff),'')
 def record(phase,outcome='ok'):states.append({'case':name,'phase':phase,'scene':observe(s),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'camera':camera(vp.camera),'outcome':outcome})
 initial_camera=camera(vp.camera);record('before');before=len(s.image_planes);undo=len(vp.history.undo_stack)
 def dismiss():
  widget=app.activeModalWidget()
  if isinstance(widget,QMessageBox):dialogs.append({'case':name,'text':widget.text()});QTest.keyClick(widget,Qt.Key_Return)
 timer=QTimer();timer.timeout.connect(dismiss);timer.start(10);win._on_import_orthophoto();timer.stop();phases=['before']
 if name=='chooser-cancel':assert len(s.image_planes)==before and len(vp.history.undo_stack)==undo;record('cancel');phases.append('cancel');saved=None
 elif name=='decode-failure-datum':assert len(s.image_planes)==before and len(vp.history.undo_stack)==undo and s.georef is not None;record('failed','decoder-error');phases.append('failed');defects.append({'case':name,'kind':'datum-published-before-failed-decode','datum':s.georef.to_dict(),'dialogs':[d for d in dialogs if d['case']==name]});saved=None
 else:
  assert len(s.image_planes)==before+1 and len(vp.history.undo_stack)==undo+1;record('import');phases.append('import');plane=s.image_planes[-1];encoded=name+'-stored.webp';shutil.copyfile(plane.path,out/encoded);cached,size=qrgba(QImage(str(plane.path)));f=name+'-cached.rgba';(out/f).write_bytes(cached);resources.append({'case':name,'file':encoded,'size':size,'rgba':f})
  assert vp.history.undo();record('undo');phases.append('undo');assert vp.history.redo();record('redo');phases.append('redo')
  if name=='nested-global-image':vp.end_group_edit();record('finish');phases.append('finish')
  saved=name+'-saved.igz';igz.save_scene(s,out/saved);loaded=Scene();igz.load_into(loaded,out/saved);assert observe(loaded)['images']==observe(s)['images']
 cases.append({'name':name,'input':initial,'tiff':tiff,'context':'leaf'if name=='nested-global-image'else'root','device_ceiling':ceiling,'initial_camera':initial_camera,'saved':saved,'phases':phases,'scope':'native-correction'if name=='decode-failure-datum'else'literal'})
 win._saved_version=s.version;win.close();app.processEvents();assert not errors,errors
payload={'source_pin':PIN,'cases':cases,'states':states,'decodes':decodes,'resources':resources,'source_defects':defects,'dialogs':dialogs,'decoder_controls':decoder_controls,'actual_source_action':'MainWindow._on_import_orthophoto','controlled_fixture_inputs':['chooser return value','device max_texture_size','initial model/datum','isolated texture cache'],'decode_cancel_scope':'API-only; actual progress dialog has no Cancel'}
(out/'original-geotiff.json').write_text(json.dumps(payload,indent=2,allow_nan=False)+'\n');print(json.dumps({'cases':len(cases),'states':len(states),'decodes':len(decodes),'resources':len(resources)}))
