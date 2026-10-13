#!/usr/bin/env python3
"""Bounded read-only actual GL axes transport at one frozen live camera pose. No image capture."""
import argparse,ast,copy,ctypes,hashlib,json,os,struct,subprocess,sys,time,traceback
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
ap=argparse.ArgumentParser();ap.add_argument("source",type=Path);ap.add_argument("output",type=Path);ap.add_argument("--requested-draw-only",action="store_true");ap.add_argument("--sample-counts",action="store_true");args=ap.parse_args();source=args.source.resolve();out=args.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,folder in [("XDG_CONFIG_HOME","config"),("XDG_DATA_HOME","data"),("XDG_CACHE_HOME","cache")]:os.environ[key]=str(out/folder)
repo=Path(__file__).resolve().parents[1];original=(repo/"ingetrazo").resolve();assert subprocess.check_output(["git","-C",str(original),"rev-parse","HEAD"],text=True).strip()==PIN
contract=json.loads((repo/"docs/audits/axis-transport/fixture-contract.json").read_text())
for row in contract["original_sources"]:assert hashlib.sha256((original/row["path"]).read_bytes()).hexdigest()==row["sha256"]
for row in contract["support_sources"]:assert hashlib.sha256((repo/row["path"]).read_bytes()).hexdigest()==row["sha256"]
manifest=json.loads((repo/"docs/audits/ai-bridge-capture/source-manifest.json").read_text());frozen=json.loads((source/"original-ai-bridge-capture.json").read_text());assert frozen["source_pin"]==PIN
case=next(c for c in frozen["cases"]if c["name"]=="default-perspective");assert case["edit_path"]==[]and case["logical_viewport"]==[800,600]
for name in ["original-ai-bridge-capture.json",case["input"]]:assert hashlib.sha256((source/name).read_bytes()).hexdigest()==manifest["members"][name]["sha256"]
sys.path.insert(0,str(original));sys.path.insert(0,str(repo/"scripts"))
from PySide6.QtGui import QVector3D as V,QImage
from PySide6.QtWidgets import QApplication
from core.dimension import Dimension
from formats import igz
from views.main_window import MainWindow
from exchange_import_observations import scene_state
app=QApplication([]);errors=[]
def exception(t,e,tb):errors.append({"type":t.__name__,"error":str(e),"traceback":"".join(traceback.format_exception(t,e,tb))});traceback.print_exception(t,e,tb)
sys.excepthook=exception
names={"rgba","overlay_record","selection_record","observation","flush"};tree=ast.parse((repo/"scripts/reference-ai-bridge-capture.py").read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names];assert len(nodes)==len(names);exec(compile(ast.Module(body=nodes,type_ignores=[]),"retained-capture-observers","exec"),globals())
host=MainWindow();v=host.viewport;igz.load_into(v.scene,source/case["input"]);v.setFixedSize(*case["logical_viewport"]);host.show();c=v.camera;pose=case["initial_camera"];c.target=V(*pose["target"]);c.distance=pose["distance"];c.yaw=pose["yaw"];c.pitch=pose["pitch"];c.perspective=pose["perspective"];c.fov_deg=pose["fov"];v.notify_scene_changed();flush(v)
assert v.context().isValid()and v._scene_fbo.isValid()and[v.width(),v.height()]==[800,600]and v.devicePixelRatioF()==1.
gl=ctypes.CDLL("libGL.so.1")
gl.glGetIntegerv.argtypes=[ctypes.c_uint,ctypes.POINTER(ctypes.c_int)];gl.glGetUniformfv.argtypes=[ctypes.c_uint,ctypes.c_int,ctypes.POINTER(ctypes.c_float)];gl.glGetUniformiv.argtypes=[ctypes.c_uint,ctypes.c_int,ctypes.POINTER(ctypes.c_int)];gl.glBindBuffer.argtypes=[ctypes.c_uint,ctypes.c_uint];gl.glGetBufferParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)];gl.glGetBufferSubData.argtypes=[ctypes.c_uint,ctypes.c_ssize_t,ctypes.c_ssize_t,ctypes.c_void_p];gl.glGetError.restype=ctypes.c_uint
gl.glBindFramebuffer.argtypes=[ctypes.c_uint,ctypes.c_uint]
gl.glGetFramebufferAttachmentParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)];gl.glBindRenderbuffer.argtypes=[ctypes.c_uint,ctypes.c_uint];gl.glGetRenderbufferParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
gl.glIsEnabled.argtypes=[ctypes.c_uint];gl.glIsEnabled.restype=ctypes.c_ubyte;gl.glGetFloatv.argtypes=[ctypes.c_uint,ctypes.POINTER(ctypes.c_float)]
rows=[];buffers={};sample_rows=[];sites=[(575,1),(749,441),(1,639)];
assert not args.sample_counts or args.requested_draw_only
original_gl=v._gl;before=copy.deepcopy(observation(host))
def integers(key,n=1):a=(ctypes.c_int*n)();gl.glGetIntegerv(key,a);return list(a)
def uniform_float(program,location,n):assert location>=0;a=(ctypes.c_float*n)();gl.glGetUniformfv(program,location,a);return {"values":list(a),"bits":list(struct.unpack("<"+"I"*n,bytes(a))),"convention":"column-major"if n==16 else"components"}
def raster_state():
 def float_values(key,n=1):a=(ctypes.c_float*n)();gl.glGetFloatv(key,a);return list(a)
 return {"depth_test":bool(gl.glIsEnabled(0x0B71)),"depth_func":integers(0x0B74)[0],"depth_write":bool(integers(0x0B72)[0]),"depth_range":float_values(0x0B70,2),"blend_enabled":bool(gl.glIsEnabled(0x0BE2)),"blend_src_rgb":integers(0x80C9)[0],"blend_dst_rgb":integers(0x80C8)[0],"blend_equation_rgb":integers(0x8009)[0],"line_width":float_values(0x0B21)[0],"line_smooth":bool(gl.glIsEnabled(0x0B20)),"multisample":bool(gl.glIsEnabled(0x809D)),"samples":integers(0x80A9)[0],"sample_alpha_to_coverage":bool(gl.glIsEnabled(0x809E)),"sample_coverage":bool(gl.glIsEnabled(0x80A0)),"scissor_test":bool(gl.glIsEnabled(0x0C11)),"scissor_box":integers(0x0C10,4),"polygon_offset_fill":bool(gl.glIsEnabled(0x8037)),"draw_framebuffer":integers(0x8CA6)[0]}
def attachment_state():
 def attachment(key,param):a=(ctypes.c_int*1)();gl.glGetFramebufferAttachmentParameteriv(0x8CA9,key,param,a);return a[0]
 def renderbuffer(param):a=(ctypes.c_int*1)();gl.glGetRenderbufferParameteriv(0x8D41,param,a);return a[0]
 previous=integers(0x8CA7)[0];result={}
 try:
  for name,key in [("color",0x8CE0),("depth",0x8D00),("stencil",0x8D20)]:
   kind=attachment(key,0x8CD0);row={"object_type":kind}
   if kind:
    row["object_name"]=attachment(key,0x8CD1)
    if kind==0x8D41:
     gl.glBindRenderbuffer(0x8D41,row["object_name"])
     row.update({"internal_format":renderbuffer(0x8D44),"width":renderbuffer(0x8D42),"height":renderbuffer(0x8D43),"samples":renderbuffer(0x8CAB)})
    elif kind==0x1702:
     row.update({"texture_level":attachment(key,0x8CD2),"layered":attachment(key,0x8DA7)})
    else:raise AssertionError("Undeclared source attachment object type")
   result[name]=row
 finally:gl.glBindRenderbuffer(0x8D41,previous)
 assert integers(0x8CA7)[0]==previous and gl.glGetError()==0
 return result
def sample_counts(arguments):
 assert args.requested_draw_only and integers(0x80A9)[0]==4
 assert not gl.glIsEnabled(0x0B90),"Diagnostic does not admit stencil-writing source state"
 gl.glGetBooleanv.argtypes=[ctypes.c_uint,ctypes.POINTER(ctypes.c_ubyte)]
 gl.glGetIntegeri_v.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
 gl.glColorMask.argtypes=[ctypes.c_ubyte]*4;gl.glDepthMask.argtypes=[ctypes.c_ubyte]
 gl.glEnable.argtypes=[ctypes.c_uint];gl.glDisable.argtypes=[ctypes.c_uint];gl.glScissor.argtypes=[ctypes.c_int]*4
 gl.glSampleMaski.argtypes=[ctypes.c_uint,ctypes.c_uint]
 gl.glGenQueries.argtypes=[ctypes.c_int,ctypes.POINTER(ctypes.c_uint)];gl.glDeleteQueries.argtypes=[ctypes.c_int,ctypes.POINTER(ctypes.c_uint)]
 gl.glBeginQuery.argtypes=[ctypes.c_uint,ctypes.c_uint];gl.glEndQuery.argtypes=[ctypes.c_uint]
 gl.glGetQueryiv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
 gl.glGetQueryObjectuiv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_uint)]
 counter_bits=(ctypes.c_int*1)();gl.glGetQueryiv(0x8914,0x8864,counter_bits);assert counter_bits[0]>0,"Actual GL sample query counter unsupported"
 current=(ctypes.c_int*1)();gl.glGetQueryiv(0x8914,0x8865,current);assert current[0]==0
 color=(ctypes.c_ubyte*4)();gl.glGetBooleanv(0x0C23,color)
 depth_write=integers(0x0B72)[0];depth_test=bool(gl.glIsEnabled(0x0B71));scissor=bool(gl.glIsEnabled(0x0C11));box=integers(0x0C10,4);mask_enabled=bool(gl.glIsEnabled(0x8E51));mask=(ctypes.c_int*1)();gl.glGetIntegeri_v(0x8E52,0,mask)
 query=(ctypes.c_uint*1)();gl.glGenQueries(1,query);assert query[0]
 result=[]
 try:
  gl.glColorMask(0,0,0,0);gl.glDepthMask(0);gl.glEnable(0x0C11);gl.glEnable(0x8E51)
  for x,top_y in sites:
   y=767-top_y
   assert not scissor or(box[0]<=x<box[0]+box[2]and box[1]<=y<box[1]+box[3])
   gl.glScissor(x,y,1,1)
   for sample in range(4):
    gl.glSampleMaski(0,1<<sample)
    counts={}
    for enabled in [True,False]:
     (gl.glEnable if enabled else gl.glDisable)(0x0B71)
     gl.glBeginQuery(0x8914,query[0]);original_gl.glDrawArrays(*arguments);gl.glEndQuery(0x8914)
     count=(ctypes.c_uint*1)();gl.glGetQueryObjectuiv(query[0],0x8866,count)
     counts["depth_enabled"if enabled else"depth_disabled"]=int(count[0])
    result.append({"site_top_origin":[x,top_y],"attachment_pixel":[x,y],"axis_first":int(arguments[1]),"sample":sample,"counts":counts})
 finally:
  gl.glColorMask(*color);gl.glDepthMask(depth_write);(gl.glEnable if depth_test else gl.glDisable)(0x0B71)
  gl.glScissor(*box);(gl.glEnable if scissor else gl.glDisable)(0x0C11)
  gl.glSampleMaski(0,mask[0]&0xffffffff);(gl.glEnable if mask_enabled else gl.glDisable)(0x8E51)
  gl.glDeleteQueries(1,query)
 assert gl.glGetError()==0
 assert raster_state()==rows[-1]["raster_state"]
 restored_color=(ctypes.c_ubyte*4)();gl.glGetBooleanv(0x0C23,restored_color);assert list(restored_color)==list(color)
 restored_mask=(ctypes.c_int*1)();gl.glGetIntegeri_v(0x8E52,0,restored_mask);assert restored_mask[0]==mask[0]and bool(gl.glIsEnabled(0x8E51))==mask_enabled
 return result
class ObservedGl:
 def __getattr__(self,name):return getattr(original_gl,name)
 def glDrawArrays(self,*arguments):
  mode,first,count=arguments
  if mode==0x0001 and integers(0x85B5)[0]==v._axes_vao.objectId():
   assert len(rows)<3 and 0<=first<3126 and count==1042
   program=integers(0x8B8D)[0];assert program==v._program.programId()
   previous=integers(0x8894)[0];buffer_id=v._axes_vbo.bufferId()
   try:
    gl.glBindBuffer(0x8892,buffer_id);length=(ctypes.c_int*1)();gl.glGetBufferParameteriv(0x8892,0x8764,length);assert length[0]==3126*12
    data=(ctypes.c_ubyte*length[0])();gl.glGetBufferSubData(0x8892,0,length[0],data);raw=bytes(data);assert gl.glGetError()==0
   finally:gl.glBindBuffer(0x8892,previous)
   digest=hashlib.sha256(raw).hexdigest();buffers[digest]=raw;clip=(ctypes.c_int*1)();gl.glGetUniformiv(program,v._loc_clip_enable,clip)
   rows.append({"mode":int(mode),"first":int(first),"count":int(count),"vao":integers(0x85B5)[0],"vbo":int(buffer_id),"vbo_sha256":digest,"mvp":uniform_float(program,v._loc_mvp,16),"color":uniform_float(program,v._loc_color,4),"opacity":uniform_float(program,v._loc_opacity,1),"clip_enabled":clip[0],"clip_plane":uniform_float(program,v._loc_clip_plane,4),"viewport":integers(0x0BA2,4),"raster_state":raster_state(),"attachments":attachment_state()})
   assert gl.glGetError()==0
   if args.sample_counts:sample_rows.extend(sample_counts(arguments))
  return original_gl.glDrawArrays(*arguments)
live_fbo=v._scene_fbo;live_size=v._fbo_size;live_depth=getattr(v,"_depth_snap",None)
try:
 v._gl=ObservedGl()
 if args.requested_draw_only:
  assert case["requests"]==[{}]
  # Exact pinned render_image GPU sequence; deliberately omit toImage/QPainter.
  v.makeCurrent();old_draw=integers(0x8CA6)[0];old_read=integers(0x8CAA)[0]
  try:
   v._export_size=(1024,768)
   try:v.paintGL()
   finally:
    v._export_size=None;v._scene_fbo=live_fbo;v._fbo_size=live_size
    gl.glBindFramebuffer(0x8CA9,old_draw);gl.glBindFramebuffer(0x8CA8,old_read)
    assert integers(0x8CA6)[0]==old_draw and integers(0x8CAA)[0]==old_read and gl.glGetError()==0
  finally:v.doneCurrent()
 else:flush(v)
finally:
 v._gl=original_gl
assert v._scene_fbo is live_fbo and v._fbo_size==live_size
if args.requested_draw_only:assert getattr(v,"_depth_snap",None)is live_depth and v._export_size is None
assert len(rows)==3 and len(buffers)==1 and not errors and observation(host)==before
assert rows[0]["mvp"]==rows[1]["mvp"]==rows[2]["mvp"]
assert all(row["viewport"]==([0,0,1024,768]if args.requested_draw_only else[0,0,800,600])and row["clip_enabled"]==0 for row in rows)
raw=next(iter(buffers.values()));(out/"source-axis-vbo.bin").write_bytes(raw)
result={"source_pin":PIN,"case":case["name"],"scope":("Actual requested 1024x768 GPU draw only, without toImage/QPainter/PNG; retained live framebuffer/depth restored." if args.requested_draw_only else "Read-only actual delegated ordinary live GL axes calls at declared frozen pose; no screenshot/render_image/PNG or source-corpus recapture."),"before":before,"after":observation(host),"readonly":True,"hook_restored":v._gl is original_gl,"callback_errors":errors,"vbo":{"file":"source-axis-vbo.bin","size":len(raw),"sha256":hashlib.sha256(raw).hexdigest(),"stride":12,"count":3126},"draws":rows,"axes_spans":v._axes_spans,"sample_counts":sample_rows,"sample_count_scope":"Precise per-sample occlusion query counts before ordinary axes draw; not stored color/depth. Color/depth writes disabled; actual accepted VBO/program/uniforms retained.","requested_gpu_draw_only":args.requested_draw_only,"live_fbo_restored":v._scene_fbo is live_fbo,"live_depth_restored":getattr(v,"_depth_snap",None)is live_depth if args.requested_draw_only else None}
(out/"source-axis-transport.json").write_text(json.dumps(result,indent=2,allow_nan=False)+"\n");host.hide();print(json.dumps({"draws":3,"vertices":3126,"readonly":True,"images_captured":0}))
