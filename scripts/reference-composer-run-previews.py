#!/usr/bin/env python3
"""Actual unchanged Qt run hover/path/item-paint producer; prereview before capture."""
import argparse,json,os,sys,subprocess,traceback
import numpy as np
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for k,n in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[k]=str(out/n)
os.environ['QT_QPA_PLATFORM']='offscreen';repo=Path(__file__).resolve().parents[1];original=repo/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QToolButton,QStyleOptionGraphicsItem
from PySide6.QtGui import QImage,QPainter,QTransform,QColor,QMouseEvent
from PySide6.QtCore import Qt,QPointF,QRectF,QEvent,qVersion
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,_SheetItem
from core.composition import Composicion
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoRunPreviewProof');app.setApplicationName('run-previews');os.chdir(out);contract=json.loads((repo/'docs/audits/composer-run-previews/fixture-contract.json').read_text());states=[];errors=[];active={'case':None,'phase':'init'}
def flush():app.processEvents();app.processEvents()
def exception(t,e,tb):
 errors.append(dict(**active,type=t.__name__,message=str(e),traceback=''.join(traceback.format_exception(t,e,tb))));(out/'source-errors.json').write_text(json.dumps(errors,indent=2)+'\n');sys.__excepthook__(t,e,tb)
sys.excepthook=exception
def xy(p):return None if p is None else[p.x(),p.y()]
def step(phase,op,**kw):return dict(phase=phase,op=op,**kw)
for recipe in contract['cases']:
 name=recipe['name'];active.update(case=name,phase='init');host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene;scene.dimension_style={'base_step_mm':11.};comp=dict(name=name,paper='A4',landscape=True,margin_mm=10.)
 if recipe['seeded']:comp['cotas']=[dict(x_mm=40.,y_mm=60.,dx_mm=50.,dy_mm=10.,sep_mm=12.,axis='h')]
 scene.compositions=[Composicion.from_dict(comp)]
 if name=='run-lifecycle':scene.compositions.append(Composicion.from_dict(dict(name='Other sheet',paper='A4',landscape=True)))
 inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile);recipe['input']=inputfile;window=ComposerWindow(host);window._auto_render=False;window._auto_timer.stop();window.resize(1100,850);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush();executed=[]
 def owner(model):
  for i,m in enumerate(window.comp.cotas):
   if m is model:return i
  return None
 def base_state():
  selected=[owner(it.model)for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)];panel=window._selected_item()
  return json.loads(json.dumps(dict(compositions=[c.to_dict()for c in scene.compositions],sheet_index=scene.compositions.index(window.comp),selection=sorted(selected),panel_owner=None if panel is None else owner(panel.model),undo=len(window.history._undo),redo=len(window.history._redo),mode=window.tool_mode,pending=dict(run_points=[xy(p)for p,h in view._chain_pts],run_sep=view._chain_sep,run_axis=view._chain_axis,run_cotas=[window.comp.cotas.index(c)for c in view._chain_cotas])),allow_nan=False))
 def preview_state():
  item=view._preview
  if item is None:return dict(exists=False,elements=[])
  path=item.path();pen=item.pen();brush=item.brush();matrix=item.sceneTransform()
  return dict(exists=True,visible=item.isVisible(),opacity=item.opacity(),z=item.zValue(),elements=[dict(type=int(path.elementAt(i).type.value),x=path.elementAt(i).x,y=path.elementAt(i).y)for i in range(path.elementCount())],pen=dict(rgba=list(pen.color().getRgb()),width=pen.widthF(),style=int(pen.style().value),cap=int(pen.capStyle().value),join=int(pen.joinStyle().value),cosmetic=pen.isCosmetic(),dash_pattern=pen.dashPattern(),dash_offset=pen.dashOffset()),brush_style=int(brush.style().value),scene_transform=[matrix.m11(),matrix.m12(),matrix.m13(),matrix.m21(),matrix.m22(),matrix.m23(),matrix.m31(),matrix.m32(),matrix.m33()])
 def observe(phase):
  before=base_state();epoch=scene.version;preview=preview_state();png=name+'-'+phase+'-preview.png';image=QImage(594,420,QImage.Format_ARGB32_Premultiplied);image.setDevicePixelRatio(1.);image.fill(QColor(255,255,255,255));painter=QPainter(image);painter.setRenderHints(view.renderHints());painter.setWorldTransform(QTransform().scale(2.,2.));painter.setClipRect(QRectF(0.,0.,297.,210.));item=view._preview
  if item is not None:
   painter.setWorldTransform(item.sceneTransform(),True);item.paint(painter,QStyleOptionGraphicsItem(),view.viewport())
  painter.end();pixels=np.frombuffer(image.constBits(),dtype=np.uint8).reshape(420,image.bytesPerLine())[:,:594*4].reshape(420,594,4);nonempty=bool(np.any(pixels[:,:,:3]!=255));assert image.save(str(out/png));viewport=name+'-'+phase+'-viewport.png';assert view.viewport().grab().save(str(out/viewport));flush();assert before==base_state()and epoch==scene.version,('Paint observer mutated source',name,phase)
  states.append(dict(case=name,phase=phase,state=before,preview=preview,paint=dict(file=png,viewport_file=viewport,density=2.,dpr=1.,clip_mm=[0.,0.,297.,210.],background=[255,255,255,255],nonempty=nonempty,render_hints=int(view.renderHints().value),page_to_device=[2.,0.,0.,0.,2.,0.,0.,0.,1.]),observer_readonly=True,source_scene_version=epoch));(out/'original-run-previews.partial.json').write_text(json.dumps(dict(cases=contract['cases'],states=states),indent=2)+'\n')
 def apply(a):
  active.update(case=name,phase=a['phase']);executed.append(a);op=a['op']
  if op in ['hover','click']:
   p=view.mapFromScene(QPointF(*a['at']));actual=view.mapToScene(p);a['effective_at']=[actual.x(),actual.y()];a['viewport_pixel']=[p.x(),p.y()];before=base_state();epoch=scene.version;QTest.mouseMove(view.viewport(),p)
   if a.get('shift',False):
    event=QMouseEvent(QEvent.MouseMove,QPointF(p),QPointF(view.viewport().mapToGlobal(p)),Qt.NoButton,Qt.NoButton,Qt.ShiftModifier);QApplication.sendEvent(view.viewport(),event)
   if op=='click':QTest.mousePress(view.viewport(),Qt.LeftButton,Qt.NoModifier,p);QTest.mouseRelease(view.viewport(),Qt.LeftButton,Qt.NoModifier,p)
   flush()
   if op=='hover':
    after=base_state();assert before['compositions']==after['compositions']and epoch==scene.version and before['undo']==after['undo']and before['redo']==after['redo'],('Hover mutated document/history',name,a['phase'])
  elif op=='arm':
   button=next(w for w in window.findChildren(QToolButton)if w.defaultAction()is window._tool_actions[a['mode']]);assert button.isEnabled();QTest.mouseClick(button,Qt.LeftButton)
  elif op=='select_fixture':window.canvas.clearSelection();window._item_for(window.comp.cotas[a['index']]).force_select();window.on_selection_changed()
  elif op=='shift':
   if a['down']:QTest.keyPress(view,Qt.Key_Shift)
   else:QTest.keyRelease(view,Qt.Key_Shift)
  elif op=='key':view.setFocus();QTest.keyClick(view,Qt.Key_Escape)
  elif op in ['undo','redo']:getattr(window,'_on_'+op)()
  elif op=='sheet_switch':window.comp_combo.setCurrentIndex(a['index'])
  elif op!='observe':raise AssertionError(op)
  flush();observe(a['phase'])
 mode='cota_base'if recipe['baseline']else'cota_cadena';actions=[step('initial','observe')]
 if recipe['seeded']:actions +=[step('selected','select_fixture',index=0)]
 actions +=[step('armed','arm',mode=mode),step('before-first','hover',at=[65.,85.])]
 if name=='run-lifecycle':
  actions +=[step('first','click',at=[40.,60.]),step('one-point','hover',at=[90.,70.]),step('cancel-pending','key'),step('cancel-tool','key'),step('rearmed','arm',mode=mode),step('a','click',at=[40.,60.]),step('b','click',at=[90.,70.]),step('offset','click',at=[65.,90.]),step('live','hover',at=[140.,90.]),step('undo','undo'),step('after-undo','hover',at=[150.,95.]),step('redo','redo'),step('after-redo','hover',at=[150.,95.]),step('new-first','click',at=[35.,140.]),step('new-second','click',at=[80.,140.]),step('new-offset','click',at=[55.,160.]),step('new-live','hover',at=[120.,150.]),step('switch-tool','arm',mode='select'),step('other-run','arm',mode=mode),step('other-first','click',at=[45.,100.]),step('other-hover','hover',at=[100.,105.]),step('switch-sheet','sheet_switch',index=1),step('after-sheet','hover',at=[130.,100.])]
 elif recipe['seeded']:
  actions +=[step('seed-next','click',at=[120.,90.]),step('live','hover',at=[150.,110.]),step('same-hover','hover',at=[150.,110.]),step('horizontal-key-only','shift',down=True),step('horizontal','hover',at=[150.,110.],shift=True),step('vertical-hover','hover',at=[125.,150.],shift=True),step('shift-up','shift',down=False),step('zero-hover','hover',at=[120.,90.]),step('before-finish','hover',at=[160.,115.]),step('finish','key'),step('after-finish','observe')]
 else:
  actions +=[step('first','click',at=[40.,60.]),step('one-point','hover',at=[75.,70.]),step('same-hover','hover',at=[75.,70.]),step('second','click',at=[90.,70.]),step('offset-hover','hover',at=[120.,80.]),step('offset-key-only','shift',down=True),step('offset-h','hover',at=[120.,80.],shift=True),step('offset-v-hover','hover',at=[43.,120.],shift=True),step('offset-shift-up','shift',down=False),step('offset-free','hover',at=[70.,95.]),step('offset-commit','click',at=[70.,95.]),step('live','hover',at=[140.,80.]),step('live-key-only','shift',down=True),step('live-h','hover',at=[140.,80.],shift=True),step('live-v-hover','hover',at=[100.,150.],shift=True),step('live-shift-up','shift',down=False),step('zero-hover','hover',at=[90.,70.]),step('jog-hover','hover',at=[135.,95.]),step('jog-commit','click',at=[135.,95.]),step('before-finish','hover',at=[170.,90.]),step('repeat-finishes','click',at=[135.,95.]),step('after-finish','observe')]
 for a in actions:apply(a)
 cohort=[r for r in states if r['case']==name];assert any(r['paint']['nonempty']and any(e['type']==1 for e in r['preview']['elements'])for r in cohort),('No operative preview paint',name)
 expected_axes={'offset-h':'h','offset-v-hover':'v','live-h':'h','live-v-hover':'v','horizontal':'h','vertical-hover':'v'}
 for i,row in enumerate(cohort):
  if row['phase'].endswith('key-only'):assert row['state']==cohort[i-1]['state']and row['preview']==cohort[i-1]['preview'],('Source key-only run changed',name,row['phase'])
  if row['phase']in expected_axes:assert row['state']['pending']['run_axis']==expected_axes[row['phase']],('Modifier ingress did not change source axis',name,row['phase'],row['state']['pending'])
 recipe['actions']=executed;active.update(case=name,phase='save');saved=name+'-original.igz';igz.save_scene(scene,out/saved);recipe['saved']=saved;active.update(case=name,phase='teardown');window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
assert not errors,errors
(out/'original-run-previews.json').write_text(json.dumps(dict(source_pin=pin,qt=qVersion(),cases=contract['cases'],states=states,source_errors=errors,scope=contract['scope']),indent=2)+'\n');print(json.dumps(dict(cases=len(contract['cases']),states=len(states))))
