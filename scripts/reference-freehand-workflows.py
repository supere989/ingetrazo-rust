#!/usr/bin/env python3
"""Missing pinned Freehand handler sessions; explicit world/screen inputs, no rendered claim."""
import os,sys,json,hashlib,subprocess,argparse
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF,Qt
from views.main_window import MainWindow
from tools.freehand import FreehandTool
from tools.base import ToolContext
from core import axes
import importlib.util
spec=importlib.util.spec_from_file_location("shape_reference",Path(__file__).with_name("reference-shape-drafting.py"));helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper);from inspect import getsource
observer_globals=vars(helper).copy();exec(getsource(helper.capture).replace("t.start_point", "getattr(t, 'start_point', None)"),observer_globals);capture=observer_globals["capture"];PIN=helper.PIN

def recipes():
 out=[]
 def case(name,points,tail=None,normal=[0,0,1],screens=None):
  screens=screens or [[100*p[0],100*p[1]+100*p[2]]for p in points]
  ops=[['press',points[0],screens[0]]]+[['move',p,s]for p,s in zip(points[1:],screens[1:])]+[['release']]+(tail or [['undo'],['redo']])
  out.append(dict(name=name,tool='freehand',normal=normal,operations=ops))
 case('sampling-below-exact-above',[[0,0,0],[.02999,0,0],[.03,0,0],[.05999,0,0],[.06,0,0],[.09,.1,0]],screens=[[0,0],[2.999,0],[3,0],[5.999,0],[6,0],[9,10]])
 for h in [1.599,1.6,1.601]:case('rdp-'+str(h),[[0,0,0],[1,h/100,0],[2,0,0]],screens=[[0,0],[100,h],[200,0]])
 for h in [9.999,10,10.001]:case('closure-'+str(h),[[0,0,0],[1,0,0],[1,1,0],[0,1,0],[h/100,0,0]],screens=[[0,0],[100,0],[100,100],[0,100],[h,0]])
 case('short-stroke-noop',[[0,0,0],[1,0,0]])
 case('vertical-plane',[[0,0,0],[1,0,0],[1,0,1],[0,0,1],[0,0,0]],normal=[0,-1,0])
 case('oblique-plane',[[0,0,0],[1,-1,0],[1,0,-1],[0,1,-1],[0,0,0]],normal=[1,1,1],screens=[[0,0],[100,0],[100,100],[0,100],[0,0]])
 pts=[[0,0,0],[1,0,0],[1,1,0],[0,1,0],[0,0,0]]
 case('cancel-release-repeat',pts,tail=[['press',[3,0,0],[300,0]],['move',[4,0,0],[400,0]],['cancel'],['release'],['press',[3,0,0],[300,0]],['move',[4,0,0],[400,0]],['move',[4,1,0],[400,100]],['release'],['undo'],['redo']])
 case('cancel-active-repeat',pts,tail=[['press',[3,0,0],[300,0]],['move',[4,0,0],[400,0]],['move',[4,1,0],[400,100]],['cancel'],['press',[3,0,0],[300,0]],['move',[4,0,0],[400,0]],['move',[4,1,0],[400,100]],['release'],['undo'],['redo'],['undo']])
 return out

def repeat_recipes():
 out=[]
 for tool in ('line','rectangle','freehand'):
  ops=[['press',[0,0,0],[0,0]],['move',[1,1,0],[100,100]],['select'],['repeat'],['press',[2,0,0],[200,0]],['move',[3,0,0],[300,0]]]
  if tool=='freehand':ops += [['move',[3,1,0],[300,100]],['release']]
  elif tool=='rectangle':ops += [['move',[3,1,0],[300,100]],['press',[3,1,0],[300,100]]]
  else:ops += [['press',[3,0,0],[300,0]]]
  ops += [['cancel'],['undo'],['redo']]
  out.append(dict(name=tool+'-select-repeat-command',tool=tool,normal=[0,0,1],operations=ops))
 return out

def main():
 ap=argparse.ArgumentParser();ap.add_argument('output');ap.add_argument('--repeat-command',action='store_true');a=ap.parse_args();assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN
 app=QApplication.instance()or QApplication([]);win=MainWindow();vp=win.viewport;vp.resize(1000,700);vp.flash_status=lambda *a,**k:None
 if a.repeat_command:
  vp._world_to_pixel=lambda p:(p.x()*100,p.y()*100+p.z()*100)
  vp._pixel_to_ray=lambda x,y:(V(x/100,y/100,10),V(0,0,-1))
 rows=[]
 for r in (repeat_recipes()if a.repeat_command else recipes()):
  vp.scene.mesh.clear();vp.scene.groups.clear();vp.history.undo_stack.clear();vp.history.redo_stack.clear();axes.sync(None);vp._last_work_plane=(V(0,0,0),V(*r['normal']));vp.axis_lock=None
  key=r['tool'];win._tools[key]=type(win._tools[key])();win._activate_tool(key);t=vp.active_tool
  vp.pick_face_placement=lambda *a:(None,None)
  def observed():
   s=capture(vp,t)
   if a.repeat_command:s['active_tool']=next(k for k,v in win._tools.items()if v is t)
   return s
  states=[dict(phase='activated',state=observed())]
  for i,op in enumerate(r['operations']):
   k=op[0]
   if k in ('press','move'):
    world=V(*op[1]);screen=QPointF(*op[2]);vp._world_from_pixel=lambda *a,w=world:w
    ev=SimpleNamespace(position=lambda s=screen:s,modifiers=lambda:Qt.NoModifier)
    if k=='press':vp._dispatch_tool_click(ev)
    else:t.on_hover(vp._build_ctx(ev))
   elif k=='select':win._activate_tool('select');t=vp.active_tool
   elif k=='repeat':assert win.repeat_last_command();t=vp.active_tool
   elif k=='release':t.on_release(vp)
   elif k=='cancel':t.on_cancel(vp)
   else:getattr(vp.history,k)()
   states.append(dict(phase=str(i)+'-'+k,accepted=None,state=observed()))
  rows.append(dict(recipe=r,activation=r["tool"],states=states));Path(a.output+'.partial').write_text(json.dumps(dict(source_pin=PIN,cases=rows),indent=2)+'\n')
 files=['views/main_window.py','tools/line.py','tools/rectangle.py','tools/freehand.py','tools/base.py','tools/arc.py','core/edits.py','core/history.py','views/viewport.py']
 data=dict(source_pin=PIN,source_sha256={f:hashlib.sha256((Path('/work/ingetrazo')/f).read_bytes()).hexdigest()for f in files},scope='Actual MainWindow Freehand activation and Viewport click/context handler, original hover/release/Cancel/History, controlled world/screen recipes; no renderer/desktop claim.',cases=rows)
 Path(a.output).write_text(json.dumps(data,indent=2)+'\n');print('captured',len(rows),'Freehand sessions')
if __name__=='__main__':main()
