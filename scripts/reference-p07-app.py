#!/usr/bin/env python3
"""Only missing guide-to-authoring and Unicode VCB actual source handlers."""
import argparse,json,os,sys,subprocess,hashlib
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import Qt,QPointF
from PySide6.QtGui import QVector3D as V
from views.main_window import MainWindow
from tools.base import ToolContext
from core.snap import SnapResult
from core import axes,units
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def point(p):return[round(p.x(),5),round(p.y(),5),round(p.z(),5)]
def ring(xs):return min(xs[i:]+xs[:i]for i in range(len(xs)))if xs else[]
def state(vp):
 m=vp.scene.mesh
 return{'vertices':sorted(point(x.position)for x in m.vertices),'edges':sorted(sorted([point(e.a),point(e.b)])for e in m.edges),'faces':sorted(ring([point(v)for v in f.vertices])for f in m.faces),'history':[len(vp.history.undo_stack),len(vp.history.redo_stack)]}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('guides',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args();assert not a.output.exists();a.output.mkdir();assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN
 app=QApplication([]);win=MainWindow();vp=win.viewport;vp.flash_status=lambda *a,**k:None;rows=[];source=json.loads(a.guides.read_text())
 recipes=[]
 for view in['xy','xz','yz','oblique']:
  r=next(x for x in source['cases']if x['name']==view+'-cross');c=r['source']['point'];end=[c[i]+r['u'][i]for i in range(3)]
  recipes.append({'name':'guide-line-'+view,'tool':'line','normal':r['normal'],'points':[c,end],'text':'٠,٥','guide_case':r['name']})
 recipes +=[{'name':'rectangle-locale','tool':'rectangle','normal':[0,0,1],'points':[[0,0,0],[1,1,0]],'text':'٢,٥;١,٢'}, {'name':'circle-locale-count','tool':'circle','normal':[0,0,1],'points':[[0,0,0],[1,0,0]],'text':'６','count_first':True}, {'name':'arc-locale-radius','tool':'arc','normal':[0,0,1],'points':[[0,0,0],[2,0,0]],'text':'١r','two_clicks':True}, {'name':'invalid-locale-refusal','tool':'line','normal':[0,0,1],'points':[[0,0,0],[1,1,0]],'text':'١٫٥','buffer_only':True}]
 for r in recipes:
  vp.scene.mesh.clear();vp.scene.groups.clear();vp.scene.guides.clear();vp.scene.units={'length':'m','precision':2};units.bind_scene(vp.scene);vp.history.undo_stack.clear();vp.history.redo_stack.clear();axes.sync(None);vp._last_work_plane=(V(0,0,0),V(*r['normal']));vp._value_buffer='';win._tools[r['tool']]=type(win._tools[r['tool']])();win._activate_tool(r['tool']);t=vp.active_tool;states=[{'phase':'before','state':state(vp)}]
  def numeric():
   accepted=[]
   if r.get('buffer_only'):vp._value_buffer=r['text']
   else:
    for c in r['text']:accepted.append(bool(vp._handle_value_key(N(text=lambda c=c:c,key=lambda:0,modifiers=lambda:Qt.NoModifier))))
   vp._handle_value_key(N(text=lambda:'',key=lambda:Qt.Key_Return,modifiers=lambda:Qt.NoModifier));return accepted
  def context(p):return ToolContext(vp,V(*p),QPointF(100*p[0],100*p[1]),Qt.NoModifier,SnapResult(V(*p),'intersection'if r.get('guide_case')else'none'))
  if r.get('count_first'):accepted=numeric()
  t.on_click(context(r['points'][0]));states.append({'phase':'press','state':state(vp)})
  if r.get('two_clicks'):t.on_click(context(r['points'][1]))
  else:t.on_hover(context(r['points'][1]))
  states.append({'phase':'hover','state':state(vp)})
  if r.get('count_first'):t.on_click(context(r['points'][1]))
  else:accepted=numeric()
  states.append({'phase':'commit-or-refuse','state':state(vp)});t.on_cancel(vp);states.append({'phase':'cancel','state':state(vp)});vp.history.undo();states.append({'phase':'undo','state':state(vp)});vp.history.redo();states.append({'phase':'redo','state':state(vp)});rows.append({'recipe':r,'characters_accepted':accepted,'states':states})
 (a.output/'source-app.json').write_text(json.dumps({'source_pin':PIN,'guide_source_sha256':hashlib.sha256(a.guides.read_bytes()).hexdigest(),'cases':rows,'scope':'Actual MainWindow tool activation and Tool/Viewport numeric-key handlers with explicit snapped world contexts; valid VCB text delivered character by character, invalid buffer submitted literally; no rendered/OS pointer claim.'},indent=2,ensure_ascii=False)+'\n');print(json.dumps({'cases':len(rows),'phases':sum(len(x['states'])for x in rows)}));win._saved_version=vp.scene.version;win.close()
if __name__=='__main__':main()
