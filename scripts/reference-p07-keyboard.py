#!/usr/bin/env python3
"""Only missing source VCB editing/chord transitions; no previous oracle recapture."""
import argparse,json,hashlib,os,sys,subprocess
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import Qt,QPointF
from PySide6.QtGui import QKeyEvent,QVector3D as V
from PySide6.QtCore import QEvent
from views.main_window import MainWindow
from tools.base import ToolContext
from core.snap import SnapResult
from core.scene import Scene
from core import axes,units
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def recipes():
 def chars(text):return[['text',c]for c in text]
 out=[]
 def case(name,ops,tool='line',acquire=True):out.append({'name':name,'tool':tool,'acquire':acquire,'operations':ops})
 for name,text in [('duplicate-comma','2,5,7'),('duplicate-dot','1.2.3'),('mixed-decimal','2.5,6'),('arabic-decimal-glyph','١٫٥'),('superscript-invalid','²'),('unsupported-letter','1z2'),('double-minus','--2-'),('fraction-prefix','/"\'1/2"')]:case(name,chars(text)+[['enter']])
 case('unit-mid-token',chars('1m')+[['enter']])
 case('rectangle-comma-list',chars('200,100')+[['enter']],tool='rectangle')
 case('rectangle-second-comma',chars('3,2,1')+[['enter']],tool='rectangle')
 case('backspace-unicode',chars('١٢')+[['backspace'],['text','٥'],['enter']])
 case('ctrl-backspace',chars('123')+[['backspace','ctrl'],['enter']])
 case('zero-denominator',chars('3/0')+[['enter']])
 case('chords-and-altgr',[['text','1'],['text','2','ctrl'],['text','3','alt'],['text','4','meta'],['text','5','ctrl-alt'],['text','6','shift'],['enter','ctrl']])
 case('escape-and-repeat',chars('2')+[['escape'],['escape'],['select'],['repeat'],['acquire'],['text','1'],['enter'],['undo'],['redo']])
 case('unit-empty-shortcut',[['text','m'],['select'],['repeat']],acquire=False)
 case('space-empty-select',[['text',' '],['repeat']],acquire=False)
 case('array-prefix-edit',[['text','/'],['text','3'],['backspace'],['text','2'],['escape'],['text','*'],['text','4'],['escape']],tool='move',acquire=False)
 return out

def point(p):return[round(p.x(),5),round(p.y(),5),round(p.z(),5)]
def state(win):
 vp=win.viewport;t=vp.active_tool;key=next(k for k,v in win._tools.items()if v is t);m=vp.scene.mesh
 anchor=getattr(t,'start_point',None)
 return{'buffer':vp._value_buffer,'tool':key,'anchor':point(anchor)if anchor is not None else None,'vertices':sorted(point(v.position)for v in m.vertices),'edges':sorted(sorted([point(e.a),point(e.b)])for e in m.edges),'faces':len(m.faces),'history':[len(vp.history.undo_stack),len(vp.history.redo_stack)]}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args();assert not a.output.exists();a.output.mkdir();assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN
 app=QApplication([]);win=MainWindow();vp=win.viewport;vp.flash_status=lambda *a,**k:None;rows=[]
 mods={'':Qt.NoModifier,'ctrl':Qt.ControlModifier,'alt':Qt.AltModifier,'meta':Qt.MetaModifier,'shift':Qt.ShiftModifier,'ctrl-alt':Qt.ControlModifier|Qt.AltModifier}
 for recipe in recipes():
  vp.scene=Scene();vp.history.scene=vp.scene;units.bind_scene(vp.scene);axes.sync(None);vp.history.undo_stack.clear();vp.history.redo_stack.clear();vp._value_buffer='';vp._last_work_plane=(V(0,0,0),V(0,0,1));vp.axis_lock=None;win._tools[recipe['tool']]=type(win._tools[recipe['tool']])();win._activate_tool(recipe['tool']);t=vp.active_tool
  def acquire():
   t=vp.active_tool;t.on_click(ToolContext(vp,V(0,0,0),QPointF(0,0),Qt.NoModifier,SnapResult(V(0,0,0),'none')));t.on_hover(ToolContext(vp,V(1,1 if recipe['tool']=='rectangle'else 0,0),QPointF(100,100),Qt.NoModifier,SnapResult(V(1,1 if recipe['tool']=='rectangle'else 0,0),'none')))
  if recipe['acquire']:acquire()
  states=[{'phase':'before','state':state(win)}]
  for i,op in enumerate(recipe['operations']):
   kind=op[0];handled=None;mod=mods[op[2]if kind=='text'and len(op)>2 else op[1]if kind in['enter','backspace']and len(op)>1 else'']
   if kind in['text','enter','backspace']:
    text=op[1]if kind=='text'else'';key=Qt.Key_Return if kind=='enter'else Qt.Key_Backspace if kind=='backspace'else Qt.Key_unknown
    ev=QKeyEvent(QEvent.KeyPress,key,mod,text);handled=bool(vp._handle_value_key(ev))
    if not handled and kind=='text'and not(mod&(Qt.ControlModifier|Qt.AltModifier|Qt.MetaModifier)):
     if text=='m':win._activate_tool('move')
     elif text==' ':win._activate_tool('select')
   elif kind=='escape':vp.escape()
   elif kind=='select':win._activate_tool('select')
   elif kind=='repeat':assert win.repeat_last_command()
   elif kind=='acquire':acquire()
   elif kind=='undo':win._on_undo()
   elif kind=='redo':win._on_redo()
   states.append({'phase':str(i)+'-'+kind,'handled':handled,'state':state(win)})
  rows.append({'recipe':recipe,'states':states})
 files=['views/viewport.py','views/main_window.py','tools/line.py','tools/rectangle.py','tools/move.py'];report={'source_pin':PIN,'source_files':{f:hashlib.sha256((Path('/work/ingetrazo')/f).read_bytes()).hexdigest()for f in files},'cases':rows,'scope':'Original _handle_value_key, Escape and real MainWindow activation/Repeat/History with explicit authored contexts. Bare unit/space fallback invokes existing MainWindow activation; no OS/QAction event-delivery or rendered claim.'};(a.output/'source.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n');print(json.dumps({'cases':len(rows),'states':sum(len(r['states'])for r in rows)}));win._saved_version=vp.scene.version;win.close()
if __name__=='__main__':main()
