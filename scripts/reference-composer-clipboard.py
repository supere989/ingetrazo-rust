#!/usr/bin/env python3
"""Actual sheet clipboard Qt ingress and ordered publication; review before one capture."""
import argparse,json,os,sys,subprocess,traceback,math
from dataclasses import asdict
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--case',action='append',dest='only_cases');a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,sub in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/sub)
os.environ['QT_QPA_PLATFORM']='offscreen';repo=Path(__file__).resolve().parents[1];original=Path('/work/ingetrazo');pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QComboBox,QAbstractSpinBox,QPlainTextEdit,QLineEdit
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtGui import QTransform,QVector3D,QImage,QColor
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,_SheetItem,CotaCanvasItem,EtiquetaCanvasItem,LlamadaCanvasItem
from core.composition import Composicion
import core.composition as model_types
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoClipboardProof');app.setApplicationName('clipboard');os.chdir(out)
families=['frames','texts','images','scalebars','nortes','leyendas','shapes','cotas','cotas_ang','cotas_rad','etiquetas','perfiles','niveles','llamadas','cajetin'];states=[];cases=[];readers=[];errors=[];opaque=[];active={'case':None,'phase':'init'}
class_names=['MarcoVista','TextoItem','ImagenItem','BarraEscala','FlechaNorte','Leyenda','FormaItem','CotaItem','CotaAngularItem','CotaRadialItem','EtiquetaItem','PerfilTerreno','NivelItem','LlamadaItem','Cajetin']
class_categories={name:i for i,name in enumerate(class_names)}
def flush():app.processEvents();app.processEvents()
def exception(t,e,tb):
 row=dict(**active,type=t.__name__,message=str(e),traceback=''.join(traceback.format_exception(t,e,tb)));errors.append(row);(out/'callback-errors.json').write_text(json.dumps(errors,indent=2)+'\n');sys.__excepthook__(t,e,tb)
sys.excepthook=exception
def action(phase,op,**kw):return dict(phase=phase,op=op,**kw)
def common(name):return dict(name=name,paper='A4',landscape=True,margin_mm=10.)
def mixed(name):
 d=common(name);d.update(frames=[dict(uid='frame-a',x_mm=30.,y_mm=30.,w_mm=32.,h_mm=25.,show_title=False,view_key='std:top')],texts=[dict(x_mm=80.,y_mm=30.,w_mm=28.,text='A',size_pt=12.),dict(x_mm=80.,y_mm=60.,w_mm=28.,text='B',size_pt=16.)],images=[dict(x_mm=130.,y_mm=30.,w_mm=28.,h_mm=20.,path='fixture.png')],leyendas=[dict(x_mm=180.,y_mm=30.,w_mm=28.,rows=['Layer'])],cotas=[dict(x_mm=30.,y_mm=90.,dx_mm=28.,dy_mm=0.,sep_mm=8.)],etiquetas=[dict(x_mm=130.,y_mm=80.,w_mm=28.,text='Label',ax_mm=-8.,ay_mm=-8.)],llamadas=[dict(x_mm=180.,y_mm=80.,w_mm=28.,h_mm=18.,bx_mm=36.,by_mm=-8.)]);return d
def select(f,i=0,**kw):return action(kw.pop('phase','pick-'+f),'body_click',family=f,index=i,**kw)
def box(phase,a,b,**kw):return action(phase,'box',a=a,b=b,**kw)
def ur(s):return[action(s+'-undo','undo'),action(s+'-redo','redo')]
for name in ['text-copy-cut-history','bound-cohort','cross-sheet-bindings','title-replace-window','all-fifteen','locked-hidden-copy']:
 d=common(name);acts=[action('initial','observe')];other=False
 if name=='text-copy-cut-history':
  d['texts']=[dict(x_mm=40.,y_mm=40.,w_mm=35.,text='Copied text',size_pt=14.)]
  acts +=[action('empty-paste','clipboard',key='V'),select('texts'),action('copy','clipboard',key='C'),action('blank','click',at=[270.,185.]),action('empty-copy-preserves','clipboard',key='C'),action('paste-one','clipboard',key='V'),action('paste-two','clipboard',key='V')]+ur('paste')+[action('cut','clipboard',key='X')]+ur('cut')+[action('restore-cut','undo')]
 elif name=='bound-cohort':
  d.update(frames=[dict(uid='copy-frame',x_mm=25.,y_mm=25.,w_mm=35.,h_mm=30.,show_title=False,view_key='std:top',group_id='bound')],texts=[dict(x_mm=80.,y_mm=30.,w_mm=28.,text='Scale {escala}',frame_uid='copy-frame',group_id='bound')],cotas=[dict(x_mm=30.,y_mm=90.,dx_mm=28.,dy_mm=0.,sep_mm=8.,anchor_uid='copy-frame',a_world=[0.,0.,0.],b_world=[1.,0.,0.])],etiquetas=[dict(x_mm=130.,y_mm=80.,w_mm=28.,text='Detached',ax_mm=-8.,ay_mm=-8.,anchor_uid='not-copied',a_world=[3.,4.,5.])])
  acts +=[box('all',[1.,1.],[190.,150.]),action('copy','clipboard',key='C'),action('paste-one','clipboard',key='V'),action('paste-two','clipboard',key='V')]+ur('paste')
 elif name=='cross-sheet-bindings':
  other=True;d.update(frames=[dict(uid='stays',x_mm=20.,y_mm=20.,w_mm=35.,h_mm=25.,show_title=False)],texts=[dict(x_mm=80.,y_mm=40.,w_mm=30.,text='Keep binding here',frame_uid='stays')],cotas=[dict(x_mm=30.,y_mm=90.,dx_mm=28.,dy_mm=0.,sep_mm=8.,anchor_uid='stays',a_world=[0.,0.,0.],b_world=[1.,0.,0.])])
  acts +=[select('texts',ctrl=True),select('cotas',ctrl=True),action('cut','clipboard',key='X')]+ur('cut')+[action('other','sheet_switch',index=1),action('cross-paste','clipboard',key='V'),action('same-paste','clipboard',key='V')]+ur('paste')
 elif name=='title-replace-window':
  other=True;d['cajetin']=dict(x_mm=110.,y_mm=140.,w_mm=150.,h_mm=40.,campos=[['PROYECTO','Copied project'],['CUSTOM','kept']])
  acts +=[select('cajetin'),action('copy','clipboard',key='C'),action('other','sheet_switch',index=1),action('replace','clipboard',key='V')]+ur('replace')+[action('same-paste','clipboard',key='V'),action('reopen-window','reopen_window'),action('window-paste','clipboard',key='V')]
 elif name=='all-fifteen':
  d=mixed(name);d['texts']=d['texts'][:1]
  d.update(scalebars=[dict(x_mm=20.,y_mm=155.,scale_n=1000.,segments=2)],nortes=[dict(x_mm=180.,y_mm=140.,size_mm=16.)],shapes=[dict(x_mm=230.,y_mm=30.,w_mm=20.,h_mm=16.,kind='rect',group_id='bundle')],cotas_ang=[dict(x_mm=230.,y_mm=80.,ax_mm=12.,ay_mm=0.,bx_mm=0.,by_mm=-12.,radius_mm=8.)],cotas_rad=[dict(x_mm=230.,y_mm=130.,radius_mm=8.,angle_deg=-30.)],perfiles=[dict(x_mm=80.,y_mm=140.,w_mm=40.,h_mm=20.)],niveles=[dict(x_mm=140.,y_mm=140.,size_mm=3.)],cajetin=dict(x_mm=180.,y_mm=175.,w_mm=55.,h_mm=20.))
  d['images'][0]['group_id']='bundle';d['texts'][0]['group_id']='text-group'
  acts +=[box('all',[1.,1.],[290.,207.]),action('copy','clipboard',key='C'),action('paste-one','clipboard',key='V')]+ur('paste')
 elif name=='locked-hidden-copy':
  d['texts']=[dict(x_mm=40.,y_mm=40.,w_mm=35.,text='Locked hidden',locked=True,hidden=True)]
  acts +=[action('items-select','select_fixture',family='texts',index=0),action('copy','clipboard',key='C'),action('paste-one','clipboard',key='V')]+ur('paste')
 cases.append(dict(name=name,composition=d,actions=acts,other_sheet=other))
if a.only_cases:
 assert set(a.only_cases)<=set(c['name']for c in cases),'Unknown case'
 cases=[c for c in cases if c['name']in a.only_cases]
(out/'model-defaults.json').write_text(json.dumps([asdict(getattr(model_types,name)())for name in class_names],ensure_ascii=False,indent=2)+'\n')
image=QImage(8,6,QImage.Format_ARGB32);image.fill(QColor(60,120,180,255));assert image.save(str(out/'fixture.png'))
for recipe in cases:
 name=recipe['name'];active.update(case=name,phase='init');ComposerWindow._clipboard=[];ComposerWindow._clipboard_from=None;QApplication.clipboard().clear();host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene
 pts=[QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(1,1,0)];scene.mesh.add_face(pts);scene.compositions=[Composicion.from_dict(recipe['composition'])]
 if recipe['other_sheet']:scene.compositions.append(Composicion.from_dict(dict(name='Other',paper='A4',landscape=True,frames=[dict(uid='stays',x_mm=10.,y_mm=10.,w_mm=40.,h_mm=30.,show_title=False)],cajetin=dict(x_mm=110.,y_mm=140.,w_mm=150.,h_mm=40.,campos=[['PROYECTO','Old destination']]))))
 inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile)
 window=ComposerWindow(host);window._auto_render=False;window._auto_timer.stop();window.resize(1200,900);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush();executed=[]
 def owner(model):
  for family in families:
   values=[window.comp.cajetin]if family=='cajetin'else getattr(window.comp,family)
   for index,m in enumerate(values):
    if m is model:return dict(family=family,index=index)
  raise AssertionError('Unknown model')
 def item(f,i):
  model=window.comp.cajetin if f=='cajetin'else getattr(window.comp,f)[i]
  return next(it for it in window.canvas.items()if isinstance(it,_SheetItem)and it.model is model)
 def record(phase):
  panel=window._selected_item();selected=[owner(it.model)for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)]
  row=dict(case=name,phase=phase,composition=window.comp.to_dict(),all_compositions=[c.to_dict()for c in scene.compositions],sheet_index=scene.compositions.index(window.comp),selection_order=selected,system_text=QApplication.clipboard().text(),clipboard=[dict(category=class_categories[type(m).__name__],record=asdict(m))for m in ComposerWindow._clipboard],clipboard_from=next((i for i,c in enumerate(scene.compositions)if c is ComposerWindow._clipboard_from),None),selection=sorted(selected,key=lambda x:(x['family'],x['index'])),panel_owner=None if panel is None else owner(panel.model),undo=len(window.history._undo),redo=len(window.history._redo))
  states.append(json.loads(json.dumps(row,allow_nan=False)));(out/'original-clipboard.partial.json').write_text(json.dumps(dict(cases=cases,states=states),ensure_ascii=False,indent=2)+'\n')
 def modifiers(a):return(Qt.ControlModifier if a.get('ctrl')else Qt.NoModifier)|(Qt.ShiftModifier if a.get('shift')else Qt.NoModifier)|(Qt.AltModifier if a.get('alt')else Qt.NoModifier)
 def pointer(a):
  point=view.mapFromScene(QPointF(*a['at']));effective=view.mapToScene(point);a.update(effective_at=[effective.x(),effective.y()],viewport_pixel=[point.x(),point.y()])
  if a['op']=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,modifiers(a),point)
  elif a['op']=='release':QTest.mouseRelease(view.viewport(),Qt.LeftButton,modifiers(a),point)
  else:QTest.mouseMove(view.viewport(),point)
  flush()
 def body(it):
  rect=it.boundingRect()
  for y in range(math.ceil(rect.top()*2),math.floor(rect.bottom()*2)+1):
   for x in range(math.ceil(rect.left()*2),math.floor(rect.right()*2)+1):
    local=QPointF(x/2.,y/2.);pixel=view.mapFromScene(it.mapToScene(local));effective=view.mapToScene(pixel);q=it.mapFromScene(effective);hits=[h for h in view.items(pixel)if isinstance(h,_SheetItem)and not h.model.locked and not h.model.hidden]
    if not hits or hits[0]is not it or not rect.contains(q)or not it.shape().contains(q):continue
    if it._on_resize_handle(q):continue
    if isinstance(it,CotaCanvasItem)and(it._on_sep_handle(q)or it._label_path().contains(q)):continue
    if isinstance(it,EtiquetaCanvasItem)and it._spot_at(q)is not None:continue
    if isinstance(it,LlamadaCanvasItem)and it._on_bubble(q):continue
    return [effective.x(),effective.y()]
  raise AssertionError(('No operative body',name,owner(it.model)))
 def apply(a):
  global window,view
  active.update(case=name,phase=a['phase']);executed.append(a);op=a['op']
  if op in ['press','move','release']:pointer(a)
  elif op=='click':
   pointer(dict(a,op='move'));press=dict(a,op='press');pointer(press);pointer(dict(a,op='release'));a.update(effective_at=press['effective_at'])
  elif op=='select_fixture':window.canvas.clearSelection();item(a['family'],a['index']).force_select();window.on_selection_changed();a['scope']='Actual Items force-selection equivalent'
  elif op=='property':
   w=getattr(window,a['widget']);assert w.isEnabled();w.setFocus()
   if isinstance(w,QAbstractSpinBox):w.setValue(a['value'])
   elif isinstance(w,QPlainTextEdit):w.setPlainText(a['value'])
   else:raise AssertionError(('Unknown property widget',a['widget']))
  elif op=='clipboard':
   view.setFocus();before=window.comp.to_dict();undo=len(window.history._undo);clip_before=[asdict(m)for m in ComposerWindow._clipboard];count=len([it for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)])
   if a['key']in['C','X']:a['copied_order']=[owner(it.model)for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)]
   QTest.keyClick(view,{'C':Qt.Key_C,'X':Qt.Key_X,'V':Qt.Key_V}[a['key']],Qt.ControlModifier);flush()
   if a['key']=='C':
    assert window.comp.to_dict()==before and len(window.history._undo)==undo
    assert len(ComposerWindow._clipboard)==count if count else [asdict(m)for m in ComposerWindow._clipboard]==clip_before
   elif (a['key']=='X'and not count)or(a['key']=='V'and not clip_before):assert window.comp.to_dict()==before and len(window.history._undo)==undo
   else:assert len(window.history._undo)==undo+1,('Clipboard did not commit exactly one transaction',name,a['phase'])
  elif op=='reopen_window':
   window.hide();window.deleteLater();flush();window=ComposerWindow(host);window._auto_render=False;window._auto_timer.stop();window.resize(1200,900);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush()
  elif op=='key':view.setFocus();QTest.keyClick(view,{'Delete':Qt.Key_Delete,'Escape':Qt.Key_Escape}[a['key']])
  elif op in ['undo','redo']:getattr(window,'_on_'+op)()
  elif op=='sheet_switch':window.comp_combo.setCurrentIndex(a['index'])
  elif op!='observe':raise AssertionError(op)
  flush()
  if name=='all-fifteen'and a['phase']=='all-release':
   acquired=[owner(it.model)for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)]
   assert {o['family']for o in acquired}==set(families)and len(acquired)==15,acquired
  record(a['phase'])
 for a in recipe['actions']:
  if a['op']in['body_click','body_drag']:
   it=item(a['family'],a['index']);at=body(it);before=window.comp.to_dict();history=len(window.history._undo)
   apply(dict(a,phase=a['phase']+'-press',op='press',at=at))
   end=[at[0]+a.get('delta',[0.,0.])[0],at[1]+a.get('delta',[0.,0.])[1]]
   if a['op']=='body_drag':apply(dict(a,phase=a['phase']+'-preview',op='move',at=end))
   apply(dict(a,phase=a['phase']+'-release',op='release',at=end))
   if a['op']=='body_drag':assert window.comp.to_dict()!=before and len(window.history._undo)==history+1,('Nonoperative group drag',name)
  elif a['op']=='box':
   apply(dict(a,phase=a['phase']+'-press',op='press',at=a['a']));apply(dict(a,phase=a['phase']+'-preview',op='move',at=a['b']));apply(dict(a,phase=a['phase']+'-release',op='release',at=a['b']))
  else:apply(a)
 active.update(case=name,phase='save');saved=name+'-original.igz';igz.save_scene(scene,out/saved);restored=type(scene)();igz.load_into(restored,out/saved);readers.append(dict(case=name,compositions=[c.to_dict()for c in restored.compositions]));record('save');recipe.update(input=inputfile,saved=saved,actions=executed);(out/(name+'-case.json')).write_text(json.dumps(dict(recipe=recipe,states=[s for s in states if s['case']==name],reader=readers[-1]),ensure_ascii=False,indent=2)+'\n');active.update(case=name,phase='teardown');window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
assert all(e['type']=='AttributeError'and e['message']=="'ComposerWindow' object has no attribute '_single_selected'"and 'in _on_cota_rad_props'in e['traceback']and e['case']=='all-fifteen'and e['phase']in['all-release','paste-one','paste-undo','paste-redo']for e in errors),errors
report=dict(source_pin=pin,qt=qVersion(),cases=cases,states=states,callback_errors=errors,opaque_guards=opaque,scope='Actual Copy/Cut/Paste keys, literal ordered clipboard, complete states/save. Random UUIDs require strict relational identity bijections; differing Qt selection iteration/z order remains explicit, never sorted away. No profile painting claim.')
(out/'original-clipboard.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(readers,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(cases),states=len(states),readers=len(readers))))
