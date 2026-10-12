#!/usr/bin/env python3
"""Actual Qt specialized handles, dimension runs and annotation multi-selection; review before capture."""
import argparse,json,os,sys,subprocess,traceback,math,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,sub in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/sub)
os.environ['QT_QPA_PLATFORM']='offscreen';repo=Path(__file__).resolve().parents[1];original=repo/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QToolButton,QComboBox,QCheckBox,QPlainTextEdit
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtGui import QTransform,QVector3D
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,_SheetItem,CotaCanvasItem,CotaAngularCanvasItem,CotaRadialCanvasItem,EtiquetaCanvasItem,LlamadaCanvasItem,cota_label_anchor
from core.composition import Composicion
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoSpecializedProof');app.setApplicationName('specialized');os.chdir(out)
families=['cotas','cotas_ang','cotas_rad','etiquetas','niveles','llamadas'];contract=json.loads((repo/'docs/audits/composer-specialized/fixture-contract.json').read_text());states=[];readers=[];errors=[];opaque=[];active={'case':None,'phase':'init'}
def flush():app.processEvents();app.processEvents()
radial_population_phases={'select','radius-angle-press','radius-angle-preview','radius-angle-release','radius-angle-undo','radius-angle-redo','minimum-press','minimum-preview','minimum-release','minimum-undo','minimum-redo'}
def exception(t,e,tb):
 trace=''.join(traceback.format_exception(t,e,tb));allowed=t is AttributeError and str(e)=="'ComposerWindow' object has no attribute '_single_selected'"and 'in _on_cota_rad_props'in trace and active['case']=='radial-endpoint'and active['phase']in radial_population_phases
 errors.append(dict(**active,type=t.__name__,message=str(e),traceback=trace,known_radial_population_failure=allowed));(out/'callback-errors.json').write_text(json.dumps(errors,indent=2)+'\n');sys.__excepthook__(t,e,tb)
sys.excepthook=exception
def action(phase,op,**kw):return dict(phase=phase,op=op,**kw)
def common():return dict(paper='A4',landscape=True,margin_mm=10.)
def cota(**kw):return dict(x_mm=40.,y_mm=70.,dx_mm=50.,dy_mm=0.,sep_mm=12.,**kw)
def select(family,index=0,phase='select'):return action(phase,'select_fixture',family=family,index=index)
def gesture(phase,family,handle,end,**kw):return action(phase,'gesture',family=family,index=0,handle=handle,end_local=end,**kw)
def ur(prefix):return[action(prefix+'-undo','undo'),action(prefix+'-redo','redo')]
def clicks(points,prefix='point',**kw):return[action(prefix+'-'+str(i),'click',at=p,**kw)for i,p in enumerate(points)]
for recipe in contract['cases']:
 name=recipe['name'];comp=common();actions=[action('initial','observe')];extra_sheet=False;model=False
 if name=='linear-handles':
  comp['cotas']=[cota(anchor_uid='absent',a_world=[0.,0.,0.],b_world=[1.,0.,0.])];actions +=[select('cotas'),gesture('sep','cotas','sep',[25.,18.])]+ur('sep')+[gesture('text','cotas','text',[28.,21.],relative_delta=True)]+ur('text')+[action('reset-text','button',widget='cota_text_reset'),gesture('endpoint','cotas','endpoint',[65.,7.])]+ur('endpoint')+[gesture('noop','cotas','endpoint',None),action('axis-h','property',widget='cota_axis',data='h'),gesture('sep-h','cotas','sep',[30.,22.]),action('axis-v','property',widget='cota_axis',data='v'),gesture('sep-v','cotas','sep',[19.,4.])]
 elif name=='angular-radius':comp['cotas_ang']=[dict(x_mm=80.,y_mm=90.,ax_mm=30.,ay_mm=0.,bx_mm=0.,by_mm=-30.,radius_mm=18.)];actions +=[select('cotas_ang'),gesture('radius','cotas_ang','radius',[25.,-25.])]+ur('radius')+[gesture('minimum','cotas_ang','radius',[0.,0.])]+ur('minimum')
 elif name=='radial-endpoint':comp['cotas_rad']=[dict(x_mm=80.,y_mm=90.,radius_mm=20.,angle_deg=-30.)];actions +=[select('cotas_rad'),gesture('radius-angle','cotas_rad','arc',[25.,15.])]+ur('radius-angle')+[gesture('minimum','cotas_rad','arc',[0.,0.])]+ur('minimum')
 elif name in ['label-leaders','label-wrap-width']:
  comp['etiquetas']=[dict(x_mm=100.,y_mm=90.,text='Label words wrap over several lines',w_mm=35.,ax_mm=-25.,ay_mm=-15.,anchor_uid='absent',a_world=[0.,0.,0.],leaders=[dict(ax_mm=45.,ay_mm=-10.,anchor_uid='absent',a_world=[1.,0.,0.])])];actions +=[select('etiquetas')]
  if name=='label-leaders':actions +=[gesture('primary','etiquetas','primary',[-35.,-20.])]+ur('primary')+[gesture('extra','etiquetas','extra',[55.,-20.])]+ur('extra')+[gesture('copy','etiquetas','primary',[-50.,-30.],ctrl=True)]+ur('copy')+[gesture('remove-extra','etiquetas','last-tip',[1.,1.])]+ur('remove-extra')
  else:actions +=[gesture('wrap','etiquetas','wrap',[20.,0.])]+ur('wrap')+[gesture('minimum','etiquetas','wrap',[0.,0.])]+ur('minimum')
 elif name=='callout-bubble-and-box':comp['llamadas']=[dict(x_mm=80.,y_mm=90.,w_mm=35.,h_mm=25.,bx_mm=45.,by_mm=-10.)];actions +=[select('llamadas'),gesture('bubble','llamadas','bubble',[55.,-20.])]+ur('bubble')+[gesture('box','llamadas','corner',[48.,32.])]+ur('box')
 elif name=='locked-hidden-handle-admission':
  comp['cotas']=[cota(z=1.),cota(locked=True,z=20.),cota(hidden=True,z=30.)];actions +=[action('overlap-click','handle_click',family='cotas',index=0,handle='endpoint'),gesture('captured-escape','cotas','endpoint',[65.,8.],escape_mid=True)]+ur('escape')
 elif name.startswith('chain-')or name=='baseline-fresh-and-seeded':
  mode='cota_base'if name.startswith('baseline')else'cota_cadena';actions +=[action('armed','arm',mode=mode)]
  if name=='chain-seeded-and-switch':
   comp['cotas']=[cota(axis='h')];extra_sheet=True;actions=[action('initial','observe'),select('cotas'),action('armed','arm',mode=mode)]+clicks([[115.,78.],[140.,83.]],shift=True)+[action('switch-ends','arm',mode='select'),action('new-run','arm',mode=mode)]+clicks([[30.,140.],[60.,140.],[45.,150.]])+[action('other-sheet','sheet_switch',index=1)]
  elif name=='chain-anchored-undo':
   model=True;comp['frames']=[dict(uid='host',x_mm=100.,y_mm=30.,w_mm=100.,h_mm=90.,scale_n=100.,view_key='std:top',show_title=False)];actions +=[action('snap-first','snap_click',frame='host',world=[0.,0.,0.]),action('snap-second','snap_click',frame='host',world=[1.,0.,0.]),action('offset','click',at=[150.,95.]),action('snap-next','snap_click',frame='host',world=[1.,1.,0.]),action('undo-midrun','undo'),action('repeat-next','snap_click',frame='host',world=[1.,1.,0.]),action('escape-finish','key',key='Escape'),action('escape-tool','key',key='Escape')]
  elif name=='baseline-fresh-and-seeded':
   actions +=clicks([[30.,70.],[60.,70.],[45.,60.],[95.,80.],[125.,65.]])+[action('escape','key',key='Escape'),action('select-tool','arm',mode='select'),select('cotas'),action('seeded-arm','arm',mode=mode)]+clicks([[150.,75.]],prefix='seed')+[action('switch','arm',mode='select')]+ur('baseline')
  else:actions +=clicks([[30.,70.],[60.,70.],[45.,60.],[95.,73.],[125.,73.],[125.,73.]])+ur('total')+[action('short-clear','click',at=[250.,170.]),action('short-arm','arm',mode=mode)]+clicks([[30.,140.],[60.,140.],[45.,130.]],prefix='short')+[action('short-escape','key',key='Escape')]
 elif name.startswith('multi-'):
  comp['cotas']=[cota(text_mm=3.,units='cm'),dict(x_mm=40.,y_mm=115.,dx_mm=50.,dy_mm=0.,sep_mm=15.,text_mm=4.,units='cm')];comp['etiquetas']=[dict(x_mm=130.,y_mm=65.,text='Mixed label',w_mm=30.,ax_mm=-10.,ay_mm=-10.)];comp['llamadas']=[dict(x_mm=130.,y_mm=110.,w_mm=30.,h_mm=20.)]
  if name=='multi-ctrl-properties':actions +=[action('ctrl-a','handle_click',family='cotas',index=0,handle='text',ctrl=True),action('ctrl-b','handle_click',family='cotas',index=1,handle='text',ctrl=True,wobble=1),action('ctrl-label','body_click',family='etiquetas',index=0,ctrl=True),action('units','property',widget='cota_units',value='m'),action('units-again','property',widget='cota_units',value='mm')]+ur('props')+[action('toggle-b-off','handle_click',family='cotas',index=1,handle='text',ctrl=True)]
  elif name=='multi-box-admission':
   comp['cotas'].append(dict(x_mm=210.,y_mm=55.,dx_mm=30.,sep_mm=10.,locked=True));comp['etiquetas'].append(dict(x_mm=210.,y_mm=100.,text='Hidden',hidden=True));actions +=[action('window','box',a=[20.,40.],b=[180.,150.]),action('crossing','box',a=[80.,100.],b=[20.,40.]),action('ctrl-add','box',a=[120.,40.],b=[190.,90.],ctrl=True),action('shift-toggle','box',a=[20.,40.],b=[100.,90.],shift=True),action('remove','box',a=[10.,30.],b=[280.,170.],ctrl=True,shift=True),action('blank','click',at=[270.,185.])]
  else:actions +=[action('ctrl-a','handle_click',family='cotas',index=0,handle='text',ctrl=True),action('ctrl-label','body_click',family='etiquetas',index=0,ctrl=True),action('ctrl-callout','body_click',family='llamadas',index=0,ctrl=True),gesture('group','llamadas','body',[13.,17.],relative_delta=True)]+ur('group')+[action('delete','key',key='Delete')]+ur('delete')+[action('undo-delete-final','undo')]
 recipe.update(composition=dict(name=name,**comp),actions=actions)
# Runtime below reads only this declared recipe/action data, and records actual effective ingress.
for recipe in contract['cases']:
 name=recipe['name'];active.update(case=name,phase='init');host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene
 if name=='chain-anchored-undo':
  pts=[QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(1,1,0),QVector3D(0,1,0)];scene.mesh.add_face(pts[:3]);scene.mesh.add_face([pts[0],pts[2],pts[3]])
 scene.dimension_style={'base_step_mm':11.};scene.compositions=[Composicion.from_dict(recipe['composition'])]
 if name=='chain-seeded-and-switch':scene.compositions.append(Composicion.from_dict(dict(name='Other sheet',**common())))
 opaque_model=None
 inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile);recipe['input']=inputfile
 if name=='multi-group-move-delete':
  data=(out/inputfile).read_bytes();doc=json.loads(data)if data[:2]!=b'PK'else json.loads(zipfile.ZipFile(out/inputfile).read('document.json'));known=doc['scene']['compositions'][0];known['vendor_specialized']={'value':None,'unicode':'café','flags':[True,False]};known['cotas'][0]['vendor_record']={'opaque':[1,'1',None]};opaque.append(dict(case=name,paths={'scene.compositions.0.vendor_specialized':known['vendor_specialized'],'scene.compositions.0.cotas.0.vendor_record':known['cotas'][0]['vendor_record']}));(out/inputfile).write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n');igz.load_into(scene,out/inputfile);opaque_model=scene.compositions[0].cotas[0]
 window=ComposerWindow(host);window._auto_render=False;window._auto_timer.stop();window.resize(1100,850);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush();executed=[]
 def owner(model):
  for family in families:
   for i,m in enumerate(getattr(window.comp,family)):
    if m is model:return dict(family=family,index=i)
  return None
 def xy(v):return None if v is None else[v.x(),v.y()]
 def record(phase):
  selected=[owner(it.model)for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)];assert all(x is not None for x in selected)
  panel=window._selected_item();pending=dict(start=xy(view._drag_start),second=xy(view._second_pt),angular=list(view._ang_pts),radial=xy(view._rad_centre),run_points=[xy(p)for p,h in view._chain_pts],run_sep=view._chain_sep,run_axis=view._chain_axis,run_cotas=[window.comp.cotas.index(m)for m in view._chain_cotas])
  row=dict(case=name,phase=phase,composition=window.comp.to_dict(),all_compositions=[c.to_dict()for c in scene.compositions],sheet_index=scene.compositions.index(window.comp),selection=sorted(selected,key=lambda x:(x['family'],x['index'])),panel_owner=None if panel is None else owner(panel.model),undo=len(window.history._undo),redo=len(window.history._redo),mode=window.tool_mode,pending=pending)
  states.append(json.loads(json.dumps(row,allow_nan=False)));(out/'original-specialized.partial.json').write_text(json.dumps(dict(cases=contract['cases'],states=states),ensure_ascii=False,indent=2)+'\n')
 def get_item(family,index):
  model=getattr(window.comp,family)[index];return next(it for it in window.canvas.items()if isinstance(it,_SheetItem)and it.model is model)
 def mods(a):return(Qt.ControlModifier if a.get('ctrl')else Qt.NoModifier)|(Qt.ShiftModifier if a.get('shift')else Qt.NoModifier)
 def pointer(a):
  point=view.mapFromScene(QPointF(*a['at']));effective=view.mapToScene(point);a['effective_at']=[effective.x(),effective.y()];a['viewport_pixel']=[point.x(),point.y()]
  if a['op']=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,mods(a),point)
  elif a['op']=='release':QTest.mouseRelease(view.viewport(),Qt.LeftButton,mods(a),point)
  else:QTest.mouseMove(view.viewport(),point)
  flush()
 def admitted(item,local,handle):
  if not item.boundingRect().contains(local)or not item.shape().contains(local):return False
  if isinstance(item,CotaCanvasItem):
   sep=item._on_sep_handle(local);end=item._on_resize_handle(local);text=item._label_path().contains(local)
   return{'sep':sep,'endpoint':end and not sep,'text':text and not sep and not end,'body':not(sep or end or text)}.get(handle,False)
  if isinstance(item,EtiquetaCanvasItem):
   spot=item._spot_at(local);wrap=item._on_resize_handle(local)
   return spot==0 if handle=='primary'else spot==1 if handle=='extra'else spot==len(item.model.spots())-1 if handle=='last-tip'else wrap and spot is None if handle=='wrap'else spot is None and not wrap
  if isinstance(item,CotaAngularCanvasItem):return item._on_radius_handle(local)if handle=='radius'else not item._on_radius_handle(local)
  if isinstance(item,CotaRadialCanvasItem):return item._on_arc_handle(local)if handle=='arc'else not item._on_arc_handle(local)
  if isinstance(item,LlamadaCanvasItem):return item._on_bubble(local)if handle=='bubble'else item._on_resize_handle(local)and not item._on_bubble(local)if handle=='corner'else not item._on_bubble(local)and not item._on_resize_handle(local)
  return False
 def handle_point(item,handle):
  m=item.model
  if handle=='sep':return QPointF(*item._line_mid())
  if handle=='endpoint':return QPointF(m.dx_mm,m.dy_mm)
  if handle=='radius':return QPointF(*item._arc_mid())
  if handle=='arc':return QPointF(*item._arc_end())
  if handle in ['primary','extra','last-tip']:return QPointF(*m.spots()[0 if handle=='primary'else 1 if handle=='extra'else -1])
  if handle in ['wrap','corner']:return QPointF(*item.size_mm())
  if handle=='bubble':return QPointF(m.bx_mm,m.by_mm)
  rect=item._label_path().boundingRect()if handle=='text'else item.boundingRect()
  for y in range(math.ceil(rect.top()*2),math.floor(rect.bottom()*2)+1):
   for x in range(math.ceil(rect.left()*2),math.floor(rect.right()*2)+1):
    local=QPointF(x/2.,y/2.);pixel=view.mapFromScene(item.mapToScene(local));effective=view.mapToScene(pixel);hits=[it for it in view.items(pixel)if isinstance(it,_SheetItem)and not it.model.locked and not it.model.hidden]
    if hits and hits[0]is item and admitted(item,item.mapFromScene(effective),handle):return item.mapFromScene(effective)
  raise AssertionError(('No operative handle/body',name,handle,owner(m)))
 def apply(a):
  active.update(case=name,phase=a['phase']);op=a['op'];executed.append(a)
  if op in ['press','move','release']:pointer(a)
  elif op=='click':
   pointer(dict(a,op='move'));press=dict(a,op='press');pointer(press);pointer(dict(a,op='release'));a.update(effective_at=press['effective_at'],viewport_pixel=press['viewport_pixel'])
  elif op=='arm':
   button=next(w for w in window.findChildren(QToolButton)if w.defaultAction()is window._tool_actions[a['mode']]);assert button.isEnabled();QTest.mouseClick(button,Qt.LeftButton)
  elif op=='select_fixture':
   window.canvas.clearSelection();get_item(a['family'],a['index']).force_select();window.on_selection_changed();a['scope']='Explicit Items-list-equivalent fixture selection'
  elif op=='property':
   w=getattr(window,a['widget']);assert w.isEnabled();w.setFocus()
   if isinstance(w,QComboBox):
    if 'data'in a:i=w.findData(a['data']);assert i>=0;w.setCurrentIndex(i)
    else:w.setCurrentText(a['value'])
   else:raise AssertionError('Undeclared property widget')
  elif op=='button':QTest.mouseClick(getattr(window,a['widget']),Qt.LeftButton)
  elif op=='key':
   view.setFocus();QTest.keyClick(view,{'Escape':Qt.Key_Escape,'Delete':Qt.Key_Delete}[a['key']])
  elif op in ['undo','redo']:getattr(window,'_on_'+op)()
  elif op=='sheet_switch':window.comp_combo.setCurrentIndex(a['index'])
  elif op!='observe':raise AssertionError(op)
  flush();record(a['phase'])
 for a in recipe['actions']:
  if a['op']in ['gesture','handle_click','body_click']:
   item=get_item(a['family'],a['index']);local=handle_point(item,a.get('handle','body'));page=item.mapToScene(local);pixel=view.mapFromScene(page);effective=view.mapToScene(pixel);actual_local=item.mapFromScene(effective);hits=[it for it in view.items(pixel)if isinstance(it,_SheetItem)and not it.model.locked and not it.model.hidden]
   assert hits and hits[0]is item and admitted(item,actual_local,a.get('handle','body')),('Quantized specialized admission',name,a['phase'],a.get('handle'),actual_local)
   start=[effective.x(),effective.y()];before=json.loads(json.dumps(window.comp.to_dict()));before_undo=len(window.history._undo)
   if a['op']=='gesture':
    target=a['end_local'];end=effective if target is None else item.mapToScene(actual_local+QPointF(*target)if a.get('relative_delta')else QPointF(*target));end=[end.x(),end.y()]
    apply(dict(a,phase=a['phase']+'-press',op='press',at=start));apply(dict(a,phase=a['phase']+'-preview',op='move',at=end))
    if a.get('escape_mid'):apply(action(a['phase']+'-escape','key',key='Escape'))
    apply(dict(a,phase=a['phase']+'-release',op='release',at=end))
    if target is not None:assert window.comp.to_dict()!=before and len(window.history._undo)==before_undo+1,('Nonoperative specialized gesture',name,a['phase'])
    else:assert window.comp.to_dict()==before and len(window.history._undo)==before_undo,('Noop mutated',name)
   else:
    apply(dict(a,phase=a['phase']+'-press',op='press',at=start));end=[start[0]+a.get('wobble',0)/2.,start[1]];apply(dict(a,phase=a['phase']+'-release',op='release',at=end));assert window.comp.to_dict()==before and len(window.history._undo)==before_undo
  elif a['op']=='box':
   apply(dict(a,phase=a['phase']+'-press',op='press',at=a['a']));apply(dict(a,phase=a['phase']+'-preview',op='move',at=a['b']));apply(dict(a,phase=a['phase']+'-release',op='release',at=a['b']))
  elif a['op']=='snap_click':
   f=next(f for f in window.comp.frames if f.uid==a['frame']);page=window._frame_world_to_page(f,[a['world']])[0];apply(dict(a,op='click',at=list(page)))
  else:apply(a)
 if opaque_model is not None:
  assert opaque_model in window.comp.cotas;index=window.comp.cotas.index(opaque_model);opaque[-1]['saved_paths']={'scene.compositions.0.vendor_specialized':opaque[-1]['paths']['scene.compositions.0.vendor_specialized'],'scene.compositions.0.cotas.'+str(index)+'.vendor_record':opaque[-1]['paths']['scene.compositions.0.cotas.0.vendor_record']}
 active.update(case=name,phase='save');saved=name+'-original.igz';igz.save_scene(scene,out/saved);restored=type(scene)();igz.load_into(restored,out/saved);readers.append(dict(case=name,compositions=[c.to_dict()for c in restored.compositions]));record('save');recipe.update(saved=saved,actions=executed);active.update(case=name,phase='teardown');window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
assert all(e['known_radial_population_failure']for e in errors),errors
report=dict(source_pin=pin,qt=qVersion(),cases=contract['cases'],states=states,callback_errors=errors,opaque_guards=opaque,scope='Actual specialized pointer/control and annotation-only multi-selection workflows; no physical pixel/general-sheet multi claim.')
(out/'original-specialized.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(readers,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(contract['cases']),states=len(states),readers=len(readers))))
