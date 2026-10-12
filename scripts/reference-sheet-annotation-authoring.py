#!/usr/bin/env python3
"""Actual unchanged Qt six-family authoring producer. Capture only after peer prereview."""
import argparse,json,os,subprocess,sys,traceback,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--only',nargs='+');a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/name)
os.environ['QT_QPA_PLATFORM']='offscreen';original=Path(__file__).resolve().parents[1]/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QToolButton
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtGui import QTransform
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,_SheetItem,CotaCanvasItem,CotaAngularCanvasItem,CotaRadialCanvasItem,EtiquetaCanvasItem,LlamadaCanvasItem,NivelCanvasItem
from core.composition import Composicion
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoAnnotationAuthoringProof');app.setApplicationName('annotations-authoring');os.chdir(out)
contract=json.loads((Path(__file__).resolve().parents[1]/'docs/audits/sheet-annotation-authoring/fixture-contract.json').read_text());families=['cotas','cotas_ang','cotas_rad','etiquetas','niveles','llamadas'];states=[];reopens=[];errors=[];windows=[];opaque=[]
active={'case':None,'phase':'construct'}
allowed_radial_phases={'cotas_rad-lifecycle':{'click-2','property-one','property-two','property-noop'},'mixed-visibility-locked':{'select-radial'},'radial-diameter':{'diameter','redo'}}
def fail(t,e,tb):
 text=''.join(traceback.format_exception(t,e,tb));allowed=t is AttributeError and str(e)=="'ComposerWindow' object has no attribute '_single_selected'" and 'in _on_cota_rad_props' in text and active['phase']in allowed_radial_phases.get(active['case'],set())
 errors.append(dict(case=active['case'],phase=active['phase'],type=t.__name__,message=str(e),traceback=text,known_original_radial_ui_failure=allowed));(out/'callback-errors.json').write_text(json.dumps(errors,indent=2)+'\n');sys.__excepthook__(t,e,tb)

sys.excepthook=fail
def flush():app.processEvents();app.processEvents()
def step(phase,op,**kw):return dict(phase=phase,op=op,**kw)
def body_allowed(item,p):
 if not item.boundingRect().contains(p)or not item.shape().contains(p):return False
 if item._on_resize_handle(p):return False
 if isinstance(item,CotaCanvasItem):return not item._on_sep_handle(p)and not item._label_path().contains(p)
 if isinstance(item,CotaAngularCanvasItem):
  mx,my=item._arc_mid();return abs(p.x()-mx)>3 or abs(p.y()-my)>3
 if isinstance(item,CotaRadialCanvasItem):return not item._on_arc_handle(p)
 if isinstance(item,EtiquetaCanvasItem):return all(abs(p.x()-x)>3 or abs(p.y()-y)>3 for x,y in item.model.spots())
 if isinstance(item,LlamadaCanvasItem):return not item._on_bubble(p)
 return True
contract['cases'] += [
 dict(name='cancel-degenerate',family='cotas',composition=dict(name='Canceled placements',paper='A4',landscape=True,margin_mm=10.),scripted_actions=[
 step('initial','observe'),step('cota-armed','arm',mode='cota'),step('cota-first','click',at=[40.,40.]),step('cota-degenerate-second','click',at=[40.,40.]),step('cota-cancel-pending','escape'),step('cota-cancel-tool','escape'),
 step('angular-armed','arm',mode='cota_ang'),step('angular-first','click',at=[60.,60.]),step('angular-second','click',at=[60.,60.]),step('angular-cancel-pending','escape'),step('angular-cancel-tool','escape'),
 step('radial-armed','arm',mode='cota_radio'),step('radial-first','click',at=[70.,70.]),step('radial-zero','click',at=[70.,70.],ctrl=True),step('radial-cancel-tool','escape'),
 step('label-armed','arm',mode='etiqueta'),step('label-first','click',at=[40.,40.]),step('label-cancel-pending','escape'),step('label-cancel-tool','escape'),
 step('level-armed','arm',mode='nivel'),step('level-cancel-tool','escape'),step('callout-armed','arm',mode='llamada'),step('callout-first','click',at=[50.,50.]),step('callout-degenerate-second','click',at=[50.,50.]),step('callout-cancel-pending','escape'),step('callout-cancel-tool','escape')]),
 dict(name='mixed-visibility-locked',family='cotas',composition=dict(name='Selection boundaries',paper='A4',landscape=True,margin_mm=10.,cotas=[dict(x_mm=40.,y_mm=40.,dx_mm=45.,dy_mm=0.,sep_mm=10.,locked=True,z=9.)],cotas_ang=[dict(x_mm=120.,y_mm=50.)],cotas_rad=[dict(x_mm=180.,y_mm=50.)],etiquetas=[dict(x_mm=40.,y_mm=100.,text='Hidden',hidden=True,z=50.)],niveles=[dict(x_mm=40.,y_mm=100.,z=3.)],llamadas=[dict(x_mm=180.,y_mm=100.)]),scripted_actions=[step('initial','observe'),step('locked-canvas-click','click',at=[40.,45.]),step('hidden-over-visible-click','click',at=[40.,100.]),step('list-select-locked','select',family='cotas',index=0),step('locked-property','property',widget='cota_text',value='Locked selected <>'),step('locked-delete','delete'),step('undo-locked-delete','undo'),step('select-angular','select',family='cotas_ang',index=0),step('select-radial','select',family='cotas_rad',index=0),step('select-label','select',family='etiquetas',index=0),step('select-level','select',family='niveles',index=0),step('select-callout','select',family='llamadas',index=0)]),
 dict(name='remembered-style-frame',family='cotas',composition=dict(name='Remembered placement style',paper='A4',landscape=True,margin_mm=10.,frames=[dict(uid='host',hidden=True,x_mm=20.,y_mm=20.,w_mm=180.,h_mm=100.,scale_n=50.,view_key='std:top')],cotas=[dict(x_mm=30.,y_mm=140.,dx_mm=40.,dy_mm=0.,text_pos='below',text_along='end',text_align='horizontal',ends='none')],niveles=[dict(x_mm=180.,y_mm=140.,symbol='circle',datum_m=2.,mirror=True)]),scripted_actions=[step('initial','observe'),step('select-old-cota','select',family='cotas',index=0),step('remember-cota','property',widget='cota_text',value='Remembered <>'),step('clear-before-new-cota','click',at=[250.,180.]),step('new-cota-arm','arm',mode='cota'),step('new-cota-first','click',at=[40.,50.]),step('new-cota-second','click',at=[85.,55.]),step('new-cota-offset','click',at=[60.,70.]),step('select-tool','arm',mode='select'),step('select-old-level','select',family='niveles',index=0),step('remember-level','property',widget='nv_text',value='Height {z}'),step('clear-before-new-level','click',at=[250.,180.]),step('new-level-arm','arm',mode='nivel'),step('new-level-place','click',at=[120.,70.]),step('undo-new-level','undo'),step('redo-new-level','redo')])]
contract['cases'].append(dict(name='radial-diameter',family='cotas_rad',composition=dict(name='Ctrl diameter',paper='A4',landscape=True,margin_mm=10.),scripted_actions=[step('initial','observe'),step('armed','arm',mode='cota_radio'),step('centre','click',at=[70.,70.]),step('diameter','click',at=[100.,90.],ctrl=True),step('undo','undo'),step('redo','redo')]))
if a.only:
 contract['cases']=[r for r in contract['cases']if r['name']in a.only];assert {r['name']for r in contract['cases']}==set(a.only)
for recipe in contract['cases']:
 name=recipe['name'];family=recipe['family'];active.update(case=name,phase='construct');host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene;scene.compositions=[Composicion.from_dict(recipe.get('composition',dict(name=name,paper='A4',landscape=True,margin_mm=10.,perfiles=[dict(hidden=True,z=42.)])))];inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile)
 if name=='mixed-visibility-locked':
  data=(out/inputfile).read_bytes();doc=json.loads(data)if data[:2]!=b'PK'else json.loads(zipfile.ZipFile(out/inputfile).read('document.json'));c=doc['scene']['compositions'][0];c['vendor_annotation_sheet']={'unicode':'café','value':None,'flag':True};paths={'scene.compositions.0.vendor_annotation_sheet':c['vendor_annotation_sheet']}
  for key in families:c[key][0]['vendor_annotation']={'family':key,'opaque':[1,'one',None,False]};paths['scene.compositions.0.'+key+'.0.vendor_annotation']=c[key][0]['vendor_annotation']
  (out/inputfile).write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n');opaque.append(dict(case=name,paths=paths,scope='Original dataclasses omit unknown keys; native raw preservation checked separately.'));igz.load_into(scene,out/inputfile)
 window=ComposerWindow(host);windows.append((window,host));window._auto_render=False;window._auto_timer.stop();window.resize(1100,850);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush();actions=[]
 def record(phase):
  selected=[it for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)];assert len(selected)<=1
  owner=None
  if selected:
   for key in families:
    for index,model in enumerate(getattr(window.comp,key)):
     if selected[0].model is model:owner=dict(family=key,index=index)
  def xy(v):return None if v is None else[v.x(),v.y()]
  pending=dict(start=xy(view._drag_start),second=xy(view._second_pt),angular=list(view._ang_pts),radial=xy(view._rad_centre))
  states.append(dict(case=name,phase=phase,composition=window.comp.to_dict(),selection=owner,undo=len(window.history._undo),redo=len(window.history._redo),mode=window.tool_mode,pending=pending,callback_error_count=len(errors)))
  (out/'original-annotations.partial.json').write_text(json.dumps(dict(cases=contract['cases'],states=states),ensure_ascii=False,indent=2)+'\n')
 def arm(mode):
  action=window._tool_actions[mode];button=next(w for w in window.findChildren(QToolButton)if w.defaultAction()is action);QTest.mouseClick(button,Qt.LeftButton);flush()
 def pointer(action):
  pos=view.mapFromScene(QPointF(*action['at']));effective=view.mapToScene(pos);action['effective_at']=[effective.x(),effective.y()];action['viewport_pixel']=[pos.x(),pos.y()]
  mods=Qt.ControlModifier if action.get('ctrl')else Qt.ShiftModifier if action.get('shift')else Qt.NoModifier
  if action['op']=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,mods,pos)
  elif action['op']=='release':QTest.mouseRelease(view.viewport(),Qt.LeftButton,mods,pos)
  else:QTest.mouseMove(view.viewport(),pos)
  flush()
 def act(action):
  active.update(case=name,phase=action['phase']);actions.append(action);op=action['op']
  if op in ['press','release','move']:pointer(action)
  elif op=='click':
   press=dict(action,op='press');pointer(press);pointer(dict(action,op='release'));action['effective_at']=press['effective_at'];action['viewport_pixel']=press['viewport_pixel']
  elif op=='arm':arm(action['mode'])
  elif op=='select':
   window.canvas.clearSelection();model=getattr(window.comp,action['family'])[action['index']];next(it for it in window.canvas.items()if isinstance(it,_SheetItem)and it.model is model).force_select();window.on_selection_changed();action['scope']='Declared source Items-list-equivalent selection, not canvas hit proof'
  elif op=='escape':view.setFocus();QTest.keyClick(view,Qt.Key_Escape)
  elif op=='undo':window._on_undo()
  elif op=='redo':window._on_redo()
  elif op=='delete':view.setFocus();QTest.keyClick(view,Qt.Key_Delete)
  elif op=='property':
   widget=getattr(window,action['widget']);assert widget.isEnabled(),('Disabled property widget is not operative UI',name,action['widget']);widget.setFocus();action['widget_enabled']=True
   if action.get('multiline'):widget.setPlainText(action['value'])
   else:widget.setText(action['value']);QTest.keyClick(widget,Qt.Key_Return)
  elif op!='observe':raise AssertionError(op)
  flush();record(action['phase'])
 if recipe.get('scripted_actions'):
  for action in recipe['scripted_actions']:act(action)
 else:
  act(step('initial','observe'));act(step('armed','arm',mode=recipe['tool']))
  for i,point in enumerate(recipe['creation_points']):
   act(step('click-'+str(i+1),'click',at=point,shift=family=='cotas_ang'and i==2))
  act(step('select-tool','arm',mode='select'));item=window._selected_item();assert item is not None and item.model is getattr(window.comp,family)[0],name
  widget={'cotas':'cota_text','cotas_ang':'cang_text','cotas_rad':'crad_text','etiquetas':'et_text','niveles':'nv_text','llamadas':'ll_number'}[family]
  for phase,value in [('property-one','First <> café'),('property-two','Second <>\nSecond line'if family=='etiquetas'else'Second <>'),('property-noop','Second <>\nSecond line'if family=='etiquetas'else'Second <>')]:act(step(phase,'property',widget=widget,value=value,multiline=family=='etiquetas'))
  item=window._selected_item();rect=item.shape().boundingRect();chosen=None
  for y in range(int(rect.top()*2),int(rect.bottom()*2)+1):
   for x in range(int(rect.left()*2),int(rect.right()*2)+1):
    local=QPointF(x/2.,y/2.)
    effective=view.mapToScene(view.mapFromScene(item.mapToScene(local)))
    hits=[it for it in view.items(view.mapFromScene(effective))if isinstance(it,_SheetItem)]
    if hits and hits[0]is item and body_allowed(item,item.mapFromScene(effective)):chosen=effective;break
   if chosen is not None:break
  assert chosen is not None,('No actual body point',name);start=[chosen.x(),chosen.y()];end=[start[0]+13.,start[1]+17.]
  before_move=(item.model.x_mm,item.model.y_mm);before_move_history=len(window.history._undo)
  for phase,op,pos in [('move-press','press',start),('move-preview','move',end),('move-release','release',end)]:act(step(phase,op,at=pos,source_body_hit=True))
  assert (item.model.x_mm,item.model.y_mm)!=before_move and len(window.history._undo)==before_move_history+1,('Body move did not commit actual geometry/history',name)
  for phase,op in [('undo-move','undo'),('redo-move','redo'),('delete','delete'),('undo-delete','undo'),('redo-delete','redo'),('undo-delete-final','undo')]:
   before_count=len(getattr(window.comp,family));act(step(phase,op))
   if op=='delete':assert len(getattr(window.comp,family))==before_count-1,('Actual Delete failed to remove selected family',name)
 active.update(case=name,phase='save');saved=name+'-original.igz';igz.save_scene(scene,out/saved);restored=type(scene)();igz.load_into(restored,out/saved);reopens.append(dict(case=name,composition=restored.compositions[0].to_dict()));record('save');recipe.update(input=inputfile,saved=saved,actions=actions);active.update(case=name,phase='teardown');window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
assert all(e['known_original_radial_ui_failure']for e in errors),('Unexpected actual Qt callback failure; retain partial observations and stderr',errors)
report=dict(source_pin=pin,qt=qVersion(),cases=contract['cases'],states=states,callback_errors=errors,source_ui_failure_scope='Only exact missing _single_selected AttributeError in _on_cota_rad_props and declared radial panel/property phases is retained as an original UI defect; no working radial property equality claim.',opaque_guards=opaque,scope='Actual Qt toolbar/page/widget signals/body drag/Delete and ComposerHistory. Body points explicitly exclude specialized handles. No physical pixels, multiselect or chained dimensions. Locked list selection is declared separately from actual canvas visibility tests. Hidden metadata frames exercise placement defaults, not model-view anchor synchronization.')
(out/'original-annotations.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(reopens,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(report['cases']),states=len(states),reader_pairs=len(reopens))))
