#!/usr/bin/env python3
"""Actual Qt GPU viewport screenshot tool over loopback; root executes once after review."""
import argparse,copy,hashlib,json,math,os,socket,subprocess,sys,time,traceback
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);args=ap.parse_args();out=args.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,folder in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/folder)
os.environ['INGETRAZO_AI_PORT']='0'
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
contract=json.loads((repo/'docs/audits/ai-bridge-capture/fixture-contract.json').read_text())
for r in contract['original_sources']:assert hashlib.sha256((original/r['path']).read_bytes()).hexdigest()==r['sha256']
assert hashlib.sha256((repo/contract['observation_helper']['path']).read_bytes()).hexdigest()==contract['observation_helper']['sha256']
sys.path.insert(0,str(original))
from PySide6.QtCore import qVersion
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QImage,QFontInfo
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.mesh import Mesh,Edge
from core.group import Group
from core.dimension import Dimension
from core.textlabel import TextLabel
from core.guide import Guide
from core.section import SectionPlane
from georef.geopath import GeoPath
from georef.points import GeoPoint
from formats import igz
from views.main_window import MainWindow
from plugins.ai_bridge import _Bridge
from tools.select import SelectTool
from exchange_import_observations import scene_state
app=QApplication.instance()or QApplication([]);errors=[]
sys.excepthook=lambda typ,error,tb:errors.append({'type':typ.__name__,'error':str(error),'traceback':''.join(traceback.format_exception(typ,error,tb))})
report={'source_pin':PIN,'cases':[],'states':[],'render_calls':[],'callback_errors':errors,'qt_version':qVersion(),'font_family':QFontInfo(app.font()).family(),'scope':contract['scope']}
def rgba(image):
 image=image.convertToFormat(QImage.Format_RGBA8888);return bytes(image.constBits())[:image.sizeInBytes()]
def cube(mesh):
 pts=[(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]
 for ids in [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]:mesh.add_face([V(*pts[i])for i in ids])
def overlay_record(item):
 if isinstance(item,Dimension):return {'a':list(item.a.toTuple()),'b':list(item.b.toTuple()),'offset':list(item.offset.toTuple()),'axis':item.axis,'layer':item.layer,'text':item.text}
 return item.to_dict()
def selection_record(s):
 owners=[('faces',s.mesh.faces),('edges',s.mesh.edges),('vertices',s.mesh.vertices)]+[(k,getattr(s,k,[]))for k in ['dimensions','text_labels','guides','section_planes','geo_paths','geo_points','groups']]
 result=[]
 for item in s.selection:
  matches=[(name,i)for name,items in owners for i,x in enumerate(items)if x is item]
  assert len(matches)==1,'Undeclared source selection owner'
  result.append(matches[0])
 return sorted(result)
def observation(host):
 v=host.viewport;s=v.scene;c=v.camera
 return {'selection':selection_record(s),'scene':scene_state(s),'overlays':{k:[overlay_record(x)for x in getattr(s,k,[])]for k in ['dimensions','text_labels','guides','section_planes','geo_paths','geo_points']},'camera':{'target':list(c.target.toTuple()),'distance':c.distance,'yaw':c.yaw,'pitch':c.pitch,'perspective':c.perspective,'fov':float(c.fov_deg),'aspect':c.aspect,'up':list(c.up.toTuple()),'znear':c.znear,'zfar':c.zfar,'two_point':c.two_point},'undo':len(v.history.undo_stack),'redo':len(v.history.redo_stack),'edit_context':len(getattr(s,'_edit_stack',[])),'current_file':str(host._current_path)if host._current_path else None,'dirty':host._is_dirty(),'scene_version':s.version}
def flush(v):
 end=time.monotonic()+10;prior=getattr(v,'_tick',0)
 v.update()
 while time.monotonic()<end:
  app.processEvents();time.sleep(.01)
  if v._gl is not None and v._program is not None and v._scene_fbo is not None and getattr(v,'_painted_view',None)is not None and getattr(v,'_tick',0)>prior:return
 raise AssertionError('Actual source OpenGL viewport failed to paint')
recipes=[('default-perspective',[{}],False,False,False),('clamped-and-portrait',[{'width':1,'height':1},{'width':99999,'height':64},{'width':120.9,'height':180.9}],False,False,False),('orthographic-live-depth',[{'width':640,'height':480},{'width':320,'height':640}],True,True,False),('committed-selected-overlays',[{'width':800,'height':600}],False,True,False),('active-edit-context',[{'width':640,'height':480}],False,True,True),('invalid-dimensions',[{'width':None},{'unexpected':1}],False,False,False)]
hosts=[]
try:
 for name,requests,ortho,overlays,edit in recipes:
  s=Scene();cube(s.mesh)
  child=Mesh();cube(child);g=Group(child,'Capture component');g.xform=M();g.xform.translate(4.,0.,0.);s.groups.append(g)
  if overlays:
   dim=Dimension(V(-1,0,0),V(1,0,0),V(0,-2,0));dim.bind(s);s.dimensions.append(dim)
   s.text_labels.append(TextLabel(V(0,0,0),V(0,0,3),'Visible leader'))
   s.guides.append(Guide(V(3,2,0)))
   s.section_planes.append(SectionPlane(V(0,0,-2),V(0,0,1),'Capture plane','A',False))
   s.geo_paths.append(GeoPath([V(-3,-2,0),V(-3,2,0)],False,'Capture path'))
   s.geo_points.append(GeoPoint(V(2,3,0),'P1'))
  inputfile=name+'-input.igz';igz.save_scene(s,out/inputfile)
  host=MainWindow();hosts.append(host);v=host.viewport;igz.load_into(v.scene,out/inputfile)
  host._current_path=out/'fixture-project.igz';host._saved_version=v.scene.version-1
  v.setFixedSize(800,600);host.show()
  c=v.camera;c.target=V(1,0,0);c.distance=12.;c.yaw=math.radians(45.);c.pitch=math.radians(30.);c.perspective=not ortho;c.fov_deg=45.
  if edit:v.begin_group_edit(v.scene.groups[0]);assert len(v.scene._edit_stack)>0
  if name=='committed-selected-overlays':v.scene.selection.add(v.scene.dimensions[0]);v.scene.selection.add(v.scene.text_labels[0])
  v.notify_scene_changed();flush(v)
  assert v.active_tool is None or isinstance(v.active_tool,SelectTool),'Fixture requires actual Select startup mode'
  assert not isinstance(getattr(v,'_hover_entity',None),Edge),'Hovered GL edge is outside finite fixture'
  assert not getattr(v,'_preview_groups',None),'Moving-group GL preview is outside finite fixture'
  if v.active_tool is not None:
   assert not list(v.active_tool.rubber_band_lines()),'Tool GL preview is outside finite fixture'
   preview=getattr(v.active_tool,'preview_faces',None)
   assert not callable(preview)or not list(preview()),'Tool solid preview is outside finite fixture'
  assert v.context().isValid()and v._scene_fbo.isValid()and v.devicePixelRatioF()==1.,'Source GPU context/DPR guard'
  if overlays:assert getattr(v,'_depth_snap',None)is not None,'Operative retained live annotation depth required'
  case={'name':name,'input':inputfile,'initial_file':'fixture-project.igz','initial_dirty':True,'requests':requests,'edit_path':[0]if edit else[],'initial_camera':{'target':[1.,0.,0.],'distance':12.,'yaw':math.radians(45.),'pitch':math.radians(30.),'perspective':not ortho,'fov':45.},'logical_viewport':[v.width(),v.height()],'selected_annotations':name=='committed-selected-overlays'};report['cases'].append(case)
  original_render=v.render_image
  def observed_render(width,height=None,_v=v,_host=host,_name=name,_original=original_render,**kwargs):
   v=_v;host=_host;name=_name;original_render=_original
   previous=v._scene_fbo;size=v._fbo_size;depth=getattr(v,'_depth_snap',None);before=copy.deepcopy(observation(host));tick=getattr(v,'_tick',0)
   v.makeCurrent()
   try:color=rgba(previous.toImage())
   finally:v.doneCurrent()
   image=original_render(width,height,**kwargs)
   assert image is not None and [image.width(),image.height()]==[width,height]
   assert v._scene_fbo is previous and v._fbo_size==size and getattr(v,'_depth_snap',None)is depth
   v.makeCurrent()
   try:after_color=rgba(previous.toImage())
   finally:v.doneCurrent()
   assert observation(host)==before and after_color==color,'Capture changed live target/model/history/camera'
   report['render_calls'].append({'case':name,'width':width,'height':height,'live_target_restored':True,'live_depth_retained':True,'ordinary_world_rgba_unchanged':True,'scene_readonly':True,'render_tick_before':tick,'render_tick_after':getattr(v,'_tick',0),'live_depth_size':list(depth[3:5])if depth is not None else None})
   return image
  v.render_image=observed_render
  bridge=_Bridge(v);port=bridge.start();client=socket.create_connection(('127.0.0.1',port),timeout=3);client.setblocking(False);buffer=b''
  try:
   for index,params in enumerate(requests):
    before=copy.deepcopy(observation(host));request={'id':name+'-'+str(index),'tool':'screenshot','args':params};client.sendall((json.dumps(request)+'\n').encode());end=time.monotonic()+15
    while b'\n'not in buffer:
     assert time.monotonic()<end,'Screenshot socket reply deadline';app.processEvents()
     try:chunk=client.recv(65536)
     except BlockingIOError:continue
     assert chunk;buffer+=chunk
    line,buffer=buffer.split(b'\n',1);reply=json.loads(line);assert observation(host)==before
    row={'case':name,'index':index,'request':request,'reply':reply,'before':before,'after':copy.deepcopy(observation(host))}
    if name=='invalid-dimensions':assert reply['ok']is False
    else:
     assert reply['ok']is True;result=reply['result'];path=Path(result['path']);image=QImage(str(path));assert not image.isNull()and [image.width(),image.height()]==[result['width'],result['height']]
     b=rgba(image);assert len(set(b[i:i+4]for i in range(0,len(b),4)))>1,'Operative nonempty rendered scene required'
     png=name+'-'+str(index)+'.png';raw=name+'-'+str(index)+'.rgba';(out/png).write_bytes(path.read_bytes());(out/raw).write_bytes(b)
     row['image']={'file':png,'rgba':raw,'size':[image.width(),image.height()],'encoded_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'rgba_sha256':hashlib.sha256(b).hexdigest()}
    report['states'].append(row);(out/'original-ai-bridge-capture.partial.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
  finally:client.close();bridge.stop();host.hide()
 assert errors==[] and len(report['cases'])==6 and len(report['states'])==10 and len(report['render_calls'])==8
 (out/'original-ai-bridge-capture.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n');print(json.dumps({'cases':6,'replies':10,'images':8,'actual_gpu_context':True,'source_pin':PIN}))
except BaseException:
 (out/'original-ai-bridge-capture.partial.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n');raise
