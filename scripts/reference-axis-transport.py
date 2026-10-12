#!/usr/bin/env python3
"""Bounded read-only actual GL axes transport at one frozen live camera pose. No image capture."""
import argparse,ast,copy,ctypes,hashlib,json,os,struct,subprocess,sys,time,traceback
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
ap=argparse.ArgumentParser();ap.add_argument("source",type=Path);ap.add_argument("output",type=Path);args=ap.parse_args();source=args.source.resolve();out=args.output.resolve();assert not out.exists();out.mkdir(parents=True)
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
rows=[];buffers={};original_gl=v._gl;before=copy.deepcopy(observation(host))
def integers(key,n=1):a=(ctypes.c_int*n)();gl.glGetIntegerv(key,a);return list(a)
def uniform_float(program,location,n):assert location>=0;a=(ctypes.c_float*n)();gl.glGetUniformfv(program,location,a);return {"values":list(a),"bits":list(struct.unpack("<"+"I"*n,bytes(a))),"convention":"column-major"if n==16 else"components"}
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
   rows.append({"mode":int(mode),"first":int(first),"count":int(count),"vao":integers(0x85B5)[0],"vbo":int(buffer_id),"vbo_sha256":digest,"mvp":uniform_float(program,v._loc_mvp,16),"color":uniform_float(program,v._loc_color,4),"opacity":uniform_float(program,v._loc_opacity,1),"clip_enabled":clip[0],"clip_plane":uniform_float(program,v._loc_clip_plane,4),"viewport":integers(0x0BA2,4)})
   assert gl.glGetError()==0
  return original_gl.glDrawArrays(*arguments)
try:
 v._gl=ObservedGl();flush(v)
finally:
 v._gl=original_gl
assert len(rows)==3 and len(buffers)==1 and not errors and observation(host)==before
assert rows[0]["mvp"]==rows[1]["mvp"]==rows[2]["mvp"]
assert all(row["viewport"]==[0,0,800,600]and row["clip_enabled"]==0 for row in rows)
raw=next(iter(buffers.values()));(out/"source-axis-vbo.bin").write_bytes(raw)
result={"source_pin":PIN,"case":case["name"],"scope":"Read-only actual delegated ordinary live GL axes calls at declared frozen pose; no screenshot/render_image/PNG or source-corpus recapture.","before":before,"after":observation(host),"readonly":True,"hook_restored":v._gl is original_gl,"callback_errors":errors,"vbo":{"file":"source-axis-vbo.bin","size":len(raw),"sha256":hashlib.sha256(raw).hexdigest(),"stride":12,"count":3126},"draws":rows,"axes_spans":v._axes_spans}
(out/"source-axis-transport.json").write_text(json.dumps(result,indent=2,allow_nan=False)+"\n");host.hide();print(json.dumps({"draws":3,"vertices":3126,"readonly":True,"images_captured":0}))
