#!/usr/bin/env python3
"""Missing-only actual Qt single-picture pointer/history corpus."""
import argparse,hashlib,json,os,shutil,subprocess,sys,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--only',nargs='+');a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for k,v in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[k]=str(out/v)
os.environ['QT_QPA_PLATFORM']='offscreen'
original=Path(__file__).resolve().parents[1]/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtGui import QTransform,QVector3D
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,ImageItem
from core.composition import Composicion
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoPictureGestureProof');app.setApplicationName('single-picture')
(out/'assets').mkdir();asset=original/'resources/textures/checker.png';shutil.copyfile(asset,out/'assets/checker.png');os.chdir(out)
class ProbeComposer(ComposerWindow):
 def render_frame(self,frame):pass # Only physical drawing render suppressed; snapping uses original geometry/HLR.
def action(kind,at=None,mods='none'):
 r=dict(op=kind,mods=mods)
 if at is not None:r['at']=at
 return r
base=dict(x_mm=40.,y_mm=40.,w_mm=30.,h_mm=20.,path='assets/checker.png',z=10.,list_name='Picture')
recipes=[]
def recipe(name,events,**kw):recipes.append(dict(name=name,events=events,**kw))
def drag(end=(88.,83.),start=(55.,50.),mods='none'):return [action('press',list(start),mods),action('move',[start[0]+4,start[1]+3],mods),action('move',list(end),mods),action('release',list(end),mods)]
recipe('unselected-drag',drag())
recipe('selected-drag',drag(),selected=0)
recipe('large-first-move',[action('press',[55.,50.]),action('move',[88.,83.]),action('release',[88.,83.])])
recipe('locked-only',drag(),base_flags={'locked':True})
recipe('hidden-only',drag(),base_flags={'hidden':True})
recipe('ctrl-toggle',drag(mods='ctrl'))
recipe('click-only',[action('press',[55.,50.]),action('release',[55.,50.])])
recipe('out-and-back',drag()+[action('press',[88.,83.]),action('move',[55.,50.]),action('move',[88.,83.]),action('release',[88.,83.])])
recipe('escape-drag',[action('press',[55.,50.]),action('move',[59.,53.]),action('move',[88.,83.]),action('escape'),action('move',[93.,88.]),action('release',[93.,88.])])
for name,flag in [('locked-overlay','locked'),('hidden-overlay','hidden'),('higher-z','z'),('equal-z','equal')]:
 overlay=dict(base,list_name='Overlay',z=20. if flag!='equal' else 10.)
 if flag in ['locked','hidden']:overlay[flag]=True
 recipe(name,drag(),overlay=overlay)
recipe('resize-free-shift',[action('press',[70.,60.],'shift'),action('move',[95.,74.],'shift'),action('release',[95.,74.],'shift')])
recipe('resize-minimum',[action('press',[70.,60.]),action('move',[41.,41.]),action('release',[41.,41.])])
recipe('resize-minimum-snapped',[action('press',[70.,60.]),action('move',[41.,41.]),action('release',[41.,41.])],guides_v=[49.],guides_h=[45.])
recipe('drag-page-guide',drag((56.,56.)),guides_v=[42.],guides_h=[44.])
recipe('resize-guide',[action('press',[70.,60.]),action('move',[95.,74.]),action('release',[95.,74.])],guides_v=[96.],guides_h=[75.])
recipe('drag-sibling',drag((95.,90.)),sibling=dict(base,x_mm=80.,y_mm=80.,w_mm=20.,h_mm=10.,locked=True,list_name='Snap sibling'))
for point in ['endpoint','midpoint']:recipe('drawing-'+point,[],drawing=point)
if a.only:recipes=[r for r in recipes if r['name']in a.only];assert {r['name']for r in recipes}==set(a.only)
rows=[];states=[];reopens=[];windows=[]
def flush():app.processEvents();app.processEvents()
for r in recipes:
 name=r['name'];host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene
 initial=dict(name=name,paper='A4',landscape=True,margin_mm=10.,images=[dict(base)],guides=dict(v=r.get('guides_v',[]),h=r.get('guides_h',[])))
 initial['images'][0].update(r.get('base_flags',{}))
 if 'overlay'in r:initial['images'].append(r['overlay'])
 if 'sibling'in r:initial['images'].append(r['sibling'])
 if 'drawing'in r:
  scene.add_edge(QVector3D(-2,0,0),QVector3D(2,0,0))
  initial['frames']=[dict(uid='drawing',x_mm=20.,y_mm=20.,w_mm=140.,h_mm=100.,scale_n=100.,view_key='std:top',style='vectorial',locked=True,z=0.)]
 scene.compositions=[Composicion.from_dict(initial)];inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile)
 window=ProbeComposer(host);windows.append((window,host));window._auto_render=False;window._auto_timer.stop();window._set_tool_mode('select');window.resize(1100,850);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush()
 if r.get('selected')is not None:
  model=window.comp.images[r['selected']];next(it for it in window.canvas.items()if isinstance(it,ImageItem)and it.model is model).force_select();flush()
 if 'drawing'in r:
  pts,wpts=window.frame_snap_points(window.comp.frames[0]);assert len(pts)>=3
  i=0 if r['drawing']=='endpoint'else 2;target=[float(v)for v in pts[i]];r['drawing_points']=pts.tolist();r['drawing_world']=wpts.tolist();r['drawing_target']=target
  r['events']=drag((target[0]+14.,target[1]+9.)) # proposed top-left target-1 before strict drawing snap
 def record(phase):
  selected=window._selected_item();index=next((i for i,m in enumerate(window.comp.images)if selected is not None and selected.model is m),None)
  states.append(dict(case=name,phase=phase,composition=window.comp.to_dict(),selection=index,undo=len(window.history._undo),redo=len(window.history._redo),source_version=scene.version))
 record('before')
 for i,e in enumerate(r['events']):
  mods={'none':Qt.NoModifier,'shift':Qt.ShiftModifier,'ctrl':Qt.ControlModifier}[e['mods']]
  if e['op']=='escape':QTest.keyClick(view,Qt.Key_Escape)
  else:
   pixel=view.mapFromScene(QPointF(*e['at']));mapped=view.mapToScene(pixel);e['effective_at']=[mapped.x(),mapped.y()];e['viewport_pixel']=[pixel.x(),pixel.y()]
   if e['op']=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,mods,pixel)
   elif e['op']=='release':QTest.mouseRelease(view.viewport(),Qt.LeftButton,mods,pixel)
   else:QTest.mouseMove(view.viewport(),pixel)
  flush();record(str(i)+'-'+e['op'])
 window._on_undo();flush();record('undo');window._on_redo();flush();record('redo')
 saved=name+'-original.igz';igz.save_scene(scene,out/saved);r['input']=inputfile;r['saved']=saved
 restored=type(scene)();igz.load_into(restored,out/saved);reopens.append(dict(case=name,composition=restored.compositions[0].to_dict()));record('save')
 window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
 (out/'original-gestures.partial.json').write_text(json.dumps(dict(cases=recipes,states=states),indent=2)+'\n')
report=dict(source_pin=pin,qt=qVersion(),cases=recipes,states=states,assets={'assets/checker.png':dict(size=asset.stat().st_size,sha256=hashlib.sha256(asset.read_bytes()).hexdigest())},scope='Actual QtTest viewport pointer/key events, unchanged CanvasView/ImageItem/ComposerHistory; only frame pixel rendering suppressed. Drawing snaps use actual source HLR. Single picture only; clocks diagnostic. No physical pixel parity.')
(out/'original-gestures.json').write_text(json.dumps(report,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(reopens,indent=2)+'\n');print(json.dumps(dict(cases=len(recipes),states=len(states),reopens=len(reopens))))
