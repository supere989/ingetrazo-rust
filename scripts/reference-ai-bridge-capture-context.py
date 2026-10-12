#!/usr/bin/env python3
"""Missing-only context texture/figure supplement; root owns the single original capture."""
import argparse,ast,copy,hashlib,json,math,os,socket,subprocess,sys,time,traceback
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
ap=argparse.ArgumentParser();ap.add_argument("output",type=Path);out=ap.parse_args().output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,folder in [("XDG_CONFIG_HOME","config"),("XDG_DATA_HOME","data"),("XDG_CACHE_HOME","cache")]:os.environ[key]=str(out/folder)
os.environ["INGETRAZO_AI_PORT"]="0"
repo=Path(__file__).resolve().parents[1];original=(repo/"ingetrazo").resolve();assert subprocess.check_output(["git","-C",str(original),"rev-parse","HEAD"],text=True).strip()==PIN
contract=json.loads((repo/"docs/audits/ai-bridge-capture-context/fixture-contract.json").read_text())
for row in contract["original_sources"]:assert hashlib.sha256((original/row["path"]).read_bytes()).hexdigest()==row["sha256"]
for row in contract["support_sources"]:assert hashlib.sha256((repo/row["path"]).read_bytes()).hexdigest()==row["sha256"]
sys.path.insert(0,str(original));sys.path.insert(0,str(repo/"scripts"))
from PySide6.QtCore import qVersion
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QImage,QFontInfo
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.mesh import Mesh,Edge
from core.group import Group
from core.dimension import Dimension
from formats import igz
from views.main_window import MainWindow
from plugins.ai_bridge import _Bridge
from tools.select import SelectTool
from exchange_import_observations import scene_state
# Reuse exact previously reviewed source fixture/observers without executing its six cases.
helper=repo/"scripts/reference-ai-bridge-capture.py";tree=ast.parse(helper.read_text());names={"rgba","cube","overlay_record","selection_record","observation","flush"}
functions=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names];assert len(functions)==len(names)
exec(compile(ast.Module(body=functions,type_ignores=[]),str(helper),"exec"),globals())
app=QApplication.instance()or QApplication([]);errors=[]
def exception(typ,error,tb):
 errors.append({"type":typ.__name__,"error":str(error),"traceback":"".join(traceback.format_exception(typ,error,tb))});traceback.print_exception(typ,error,tb)
sys.excepthook=exception
report={"source_pin":PIN,"cases":[],"states":[],"render_calls":[],"callback_errors":errors,"qt_version":qVersion(),"font_family":QFontInfo(app.font()).family(),"scope":contract["scope"]};hosts=[]
def persist():
 (out/"original-ai-bridge-capture.partial.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")
def textured_quad(path,name,x):
 mesh=Mesh();f=mesh.add_face([V(-1,0,0),V(-1,0,2),V(1,0,2),V(1,0,0)]);f.attrs["texture"]={"path":str(path),"sw":2.,"sh":2.}
 g=Group(mesh,name);g.xform=M();g.xform.translate(x,0.,0.);return g
try:
 for name in ["startup-billboard-context","shared-texture-context"]:
  host=MainWindow();hosts.append(host);v=host.viewport;s=v.scene
  figures=[g for g in s.groups if getattr(g,"billboard",False)];assert len(figures)==1,"Actual fresh MainWindow engineer required"
  assert any("ingeniero"in str(f.attrs.get("texture",{}).get("path",""))for f in figures[0].mesh.faces),"Actual startup asset identity required"
  if name=="shared-texture-context":
   asset=out/"brick.png";asset.write_bytes((original/"resources/textures/brick.png").read_bytes())
   subject=textured_quad(asset,"Shared subject",3.);s.groups.append(textured_quad(asset,"Shared parked",-3.));s.groups.append(subject)
   assert s.groups[-1].mesh.faces[0].attrs["texture"]["path"]==s.groups[-2].mesh.faces[0].attrs["texture"]["path"]
  else:
   mesh=Mesh();cube(mesh);subject=Group(mesh,"Capture subject");subject.xform=M();subject.xform.translate(3.,0.,0.);s.groups.append(subject)
  edit_index=s.groups.index(subject);inputfile=name+"-input.igz";igz.save_scene(s,out/inputfile)
  # Reopen the actual authored input through the ordinary loader, retaining fresh-window figure only via that file.
  igz.load_into(v.scene,out/inputfile);host._current_path=out/"fixture-project.igz";host._saved_version=v.scene.version-1
  v.setFixedSize(800,600);host.show();c=v.camera;c.target=V(1,0,1);c.distance=12.;c.yaw=math.radians(45.);c.pitch=math.radians(25.);c.perspective=True;c.fov_deg=45.
  shared=[g for g in v.scene.groups if g.name in ("Shared subject","Shared parked")]
  if name=="shared-texture-context":
   assert len(shared)==2
   shared_path=shared[0].mesh.faces[0].attrs["texture"]["path"]
   assert all(g.mesh.faces[0].attrs["texture"]["path"]==shared_path for g in shared),"Loader must retain shared texture identity"
  v.begin_group_edit(v.scene.groups[edit_index]);assert len(v.scene._edit_stack)==1
  v.notify_scene_changed();flush(v)
  assert (v.active_tool is None or isinstance(v.active_tool,SelectTool))and not isinstance(getattr(v,"_hover_entity",None),Edge)
  assert v.context().isValid()and v._scene_fbo.isValid()and v.devicePixelRatioF()==1.
  assert getattr(v,"_billboard_groups",None)and len(v._billboard_groups[1])>=1,"Actual visible startup billboard packet required"
  requests=[{"width":640,"height":480}for _ in range(3)]
  actions=[[],[{"op":"camera_angles","yaw":math.radians(75.),"pitch":math.radians(25.)}],[{"op":"finish_edit"}]]
  case={"name":name,"input":inputfile,"initial_file":"fixture-project.igz","initial_dirty":True,"requests":requests,"actions_before":actions,"edit_path":[edit_index],"initial_camera":{"target":[1.,0.,1.],"distance":12.,"yaw":math.radians(45.),"pitch":math.radians(25.),"perspective":True,"fov":45.},"logical_viewport":[800,600],"selected_annotations":False};report["cases"].append(case)
  original_render=v.render_image
  def observed_render(width,height=None,_v=v,_host=host,_name=name,_original=original_render,**kwargs):
   previous=_v._scene_fbo;size=_v._fbo_size;depth=getattr(_v,"_depth_snap",None);before=copy.deepcopy(observation(_host));_v.makeCurrent()
   try:color=rgba(previous.toImage())
   finally:_v.doneCurrent()
   image=_original(width,height,**kwargs);assert image is not None and [image.width(),image.height()]==[width,height]
   assert _v._scene_fbo is previous and _v._fbo_size==size and getattr(_v,"_depth_snap",None)is depth
   _v.makeCurrent()
   try:after=rgba(previous.toImage())
   finally:_v.doneCurrent()
   assert observation(_host)==before and after==color
   report["render_calls"].append({"case":_name,"width":width,"height":height,"live_target_restored":True,"live_depth_retained":True,"ordinary_world_rgba_unchanged":True,"scene_readonly":True});return image
  v.render_image=observed_render;bridge=_Bridge(v);port=bridge.start();client=socket.create_connection(("127.0.0.1",port),timeout=3);client.setblocking(False);buffer=b""
  try:
   for index,params in enumerate(requests):
    for action in actions[index]:
     if action["op"]=="camera_angles":
      geometry=scene_state(v.scene);undo=len(v.history.undo_stack);redo=len(v.history.redo_stack);v.camera.yaw=action["yaw"];v.camera.pitch=action["pitch"];assert scene_state(v.scene)==geometry and len(v.history.undo_stack)==undo and len(v.history.redo_stack)==redo
     elif action["op"]=="finish_edit":v.end_one_group_edit();assert not v.scene._edit_stack
     else:raise AssertionError("Undeclared action")
    v.notify_scene_changed();flush(v)
    if name=="shared-texture-context":
     eye=v.camera.eye()
     for g in shared:
      face=g.mesh.faces[0];points=[vertex.position for vertex in face.loop];normal=face.normal()
      if g.xform is not None:points=[g.xform.map(point)for point in points];normal=g.xform.mapVector(normal)
      centre=sum(points,V())/len(points);assert V.dotProduct(normal,eye-centre)>0,"Actual textured front must face camera"
     count=sum(count for (path,shade),start,count in v._tex_runs if path==shared_path)
     for mesh,paint,groups,mirrored in getattr(v,"_frame_instanced",{}).values():
      actual=[g for g in groups if g.name in ("Shared subject","Shared parked")]
      if actual:
       entries=[entry for (mesh_id,signature),entry in getattr(v,"_proto_draw",{}).items()if mesh_id==id(mesh)]
       assert len(entries)==1,"Actual retained sampled prototype required"
       count+=sum(count for (path,shade),start,count in entries[0]["tex_runs"]if path==shared_path)*len(actual)
     assert count>=12 and getattr(v,"_tex_cache",{}).get(shared_path)is not None,"Both actual shared quads must reach sampled packets"
    before=copy.deepcopy(observation(host));request={"id":name+"-"+str(index),"tool":"screenshot","args":params};client.sendall((json.dumps(request)+"\n").encode());end=time.monotonic()+15
    while b"\n"not in buffer:
     assert time.monotonic()<end,"Actual socket deadline";app.processEvents()
     try:chunk=client.recv(65536)
     except BlockingIOError:continue
     assert chunk;buffer+=chunk
    line,buffer=buffer.split(b"\n",1);reply=json.loads(line);assert reply["ok"]is True and observation(host)==before
    path=Path(reply["result"]["path"]);image=QImage(str(path));assert not image.isNull()and[image.width(),image.height()]==[640,480];raw=rgba(image);assert len(set(raw[i:i+4]for i in range(0,len(raw),4)))>1
    png=name+"-"+str(index)+".png";rgba_file=name+"-"+str(index)+".rgba";(out/png).write_bytes(path.read_bytes());(out/rgba_file).write_bytes(raw)
    report["states"].append({"case":name,"index":index,"request":request,"reply":reply,"before":before,"after":copy.deepcopy(observation(host)),"image":{"file":png,"rgba":rgba_file,"size":[640,480],"encoded_sha256":hashlib.sha256(path.read_bytes()).hexdigest(),"rgba_sha256":hashlib.sha256(raw).hexdigest()}});persist()
  finally:client.close();bridge.stop();host.hide()
 assert not errors and len(report["cases"])==2 and len(report["states"])==6 and len(report["render_calls"])==6
 (out/"original-ai-bridge-capture.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n");print(json.dumps({"cases":2,"replies":6,"images":6,"source_pin":PIN}))
except BaseException:persist();raise
