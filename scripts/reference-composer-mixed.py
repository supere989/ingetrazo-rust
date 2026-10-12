#!/usr/bin/env python3
"""Actual heterogeneous sheet Qt ingress; frozen once after prereview."""
import argparse,json,os,sys,subprocess,traceback,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--only');a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,sub in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/sub)
os.environ['QT_QPA_PLATFORM']='offscreen';repo=Path(__file__).resolve().parents[1];original=Path('/work/ingetrazo');pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QComboBox,QAbstractSpinBox,QPlainTextEdit,QLineEdit
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtGui import QTransform,QVector3D,QImage,QColor
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,_SheetItem,CotaCanvasItem,EtiquetaCanvasItem,LlamadaCanvasItem
from core.composition import Composicion
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoMixedProof');app.setApplicationName('mixed');os.chdir(out)
families=['frames','texts','images','scalebars','nortes','leyendas','shapes','cotas','cotas_ang','cotas_rad','etiquetas','perfiles','niveles','llamadas','cajetin'];states=[];cases=[];readers=[];errors=[];opaque=[];active={'case':None,'phase':'init'}
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
for name in ['all-kinds-box','mixed-ctrl-move','same-kind-properties','mixed-delete-history','group-id-and-alt-stack','locked-hidden-list','sheet-switch-history','bound-frame-followers']:
 d=mixed(name);acts=[action('initial','observe')];other=False
 if name=='all-kinds-box':
  d.update(scalebars=[dict(x_mm=20.,y_mm=155.,scale_n=1000.,segments=2)],nortes=[dict(x_mm=180.,y_mm=140.,size_mm=16.)],shapes=[dict(x_mm=230.,y_mm=30.,w_mm=20.,h_mm=16.,kind='rect')],cotas_ang=[dict(x_mm=230.,y_mm=80.,ax_mm=12.,ay_mm=0.,bx_mm=0.,by_mm=-12.,radius_mm=8.)],cotas_rad=[dict(x_mm=230.,y_mm=130.,radius_mm=8.,angle_deg=-30.)],perfiles=[dict(x_mm=80.,y_mm=140.,w_mm=40.,h_mm=20.)],niveles=[dict(x_mm=140.,y_mm=140.,size_mm=3.)],cajetin=dict(x_mm=180.,y_mm=175.,w_mm=55.,h_mm=20.))
  acts +=[box('all',[1.,1.],[290.,207.]),box('corner',[75.,40.],[1.,1.]),box('add',[70.,20.],[112.,75.],ctrl=True),box('toggle',[125.,20.],[165.,58.],shift=True),box('subtract',[1.,1.],[290.,207.],ctrl=True,shift=True),action('save-empty','observe')]
 elif name=='mixed-ctrl-move':
  acts +=[select('frames',ctrl=True),select('texts',ctrl=True),select('images',ctrl=True),select('leyendas',ctrl=True),select('cotas',ctrl=True),select('etiquetas',ctrl=True),select('llamadas',ctrl=True),action('group','body_drag',family='frames',index=0,delta=[13.,17.])]+ur('group')
 elif name=='same-kind-properties':
  acts +=[select('texts',ctrl=True),select('images',ctrl=True),select('texts',1,ctrl=True),action('size','property',widget='text_size',value=20.),action('size-again','property',widget='text_size',value=22.),action('body','property',widget='text_edit',value='Changed')]+ur('props')
 elif name=='mixed-delete-history':
  acts +=[box('select-all',[1.,1.],[260.,130.]),action('delete','key',key='Delete')]+ur('delete')+[action('restore','undo')]
 elif name=='group-id-and-alt-stack':
  for f in ['frames','texts','images']:d[f][0]['group_id']='group-a'
  acts +=[select('images'),action('blank','click',at=[270.,185.])]
  d['texts'].append(dict(x_mm=130.,y_mm=30.,w_mm=28.,text='Top',size_pt=12.,z=30.));acts +=[action('top','click',at=[140.,33.]),action('beneath','click',at=[140.,33.],alt=True),action('beneath-again','click',at=[140.,33.],alt=True)]
 elif name=='locked-hidden-list':
  d['images'][0]['locked']=True;d['texts'][1]['hidden']=True
  acts +=[box('visible',[1.,1.],[260.,130.]),action('clear','click',at=[270.,185.]),action('list-locked','select_fixture',family='images',index=0),action('opacity','property',widget='img_opacity',value=63),action('delete-locked','key',key='Delete'),action('undo-locked','undo'),action('list-hidden','select_fixture',family='texts',index=1)]
 elif name=='sheet-switch-history':
  other=True;acts +=[select('frames',ctrl=True),select('images',ctrl=True),action('group','body_drag',family='images',index=0,delta=[11.,13.]),action('other-sheet','sheet_switch',index=1),action('other-select','click',at=[20.,20.]),action('back','sheet_switch',index=0),action('undo-on-return','undo'),action('redo-on-return','redo')]
 elif name=='bound-frame-followers':
  d['texts'][0].update(frame_uid='frame-a',follow=True,text='{escala}');d['cotas'][0].update(anchor_uid='frame-a',a_world=[0.,0.,0.],b_world=[1.,0.,0.]);acts +=[select('frames',ctrl=True),select('images',ctrl=True),action('move-frame','body_drag',family='frames',index=0,delta=[9.,11.])]+ur('frame')
 cases.append(dict(name=name,composition=d,actions=acts,other_sheet=other))
image=QImage(8,6,QImage.Format_ARGB32);image.fill(QColor(60,120,180,255));assert image.save(str(out/'fixture.png'))
if a.only:cases=[r for r in cases if r['name']in a.only.split(',')]
for recipe in cases:
 name=recipe['name'];active.update(case=name,phase='init');host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene
 pts=[QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(1,1,0)];scene.mesh.add_face(pts);scene.compositions=[Composicion.from_dict(recipe['composition'])]
 if recipe['other_sheet']:scene.compositions.append(Composicion.from_dict(dict(name='Other',paper='A4',landscape=True,frames=[dict(x_mm=10.,y_mm=10.,w_mm=40.,h_mm=30.,show_title=False)])))
 inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile)
 opaque_model=None
 if name=='mixed-delete-history':
  import zipfile
  data=(out/inputfile).read_bytes()
  if data[:2]==b'PK':
   with zipfile.ZipFile(out/inputfile)as z:doc=json.loads(z.read('document.json'))
  else:doc=json.loads(data)
  compdoc=doc['scene']['compositions'][0];compdoc['vendor_mixed']={'epoch':1770000000,'keep':[1,'1',None]};compdoc['images'][0]['vendor_mixed_image']={'immutable':[True,False]}
  opaque.append(dict(case=name,paths={'scene.compositions.0.vendor_mixed':compdoc['vendor_mixed'],'scene.compositions.0.images.0.vendor_mixed_image':compdoc['images'][0]['vendor_mixed_image']}))
  (out/inputfile).write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n');igz.load_into(scene,out/inputfile);opaque_model=scene.compositions[0].images[0]
 image_before=dict(vars(scene.compositions[0].images[0]));window=ComposerWindow(host);window._auto_render=False;window._auto_timer.stop();window.resize(1200,900);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush();executed=[]
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
  row=dict(case=name,phase=phase,composition=window.comp.to_dict(),all_compositions=[c.to_dict()for c in scene.compositions],sheet_index=scene.compositions.index(window.comp),selection_order=selected,selection=sorted(selected,key=lambda x:(x['family'],x['index'])),panel_owner=None if panel is None else owner(panel.model),undo=len(window.history._undo),redo=len(window.history._redo))
  states.append(json.loads(json.dumps(row,allow_nan=False)));(out/'original-mixed.partial.json').write_text(json.dumps(dict(cases=cases,states=states),ensure_ascii=False,indent=2)+'\n')
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
  elif op=='key':view.setFocus();QTest.keyClick(view,{'Delete':Qt.Key_Delete,'Escape':Qt.Key_Escape}[a['key']])
  elif op in ['undo','redo']:getattr(window,'_on_'+op)()
  elif op=='sheet_switch':window.comp_combo.setCurrentIndex(a['index'])
  elif op!='observe':raise AssertionError(op)
  flush()
  if name=='all-kinds-box'and a['phase']=='all-release':
   selected=[owner(it.model)for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)]
   assert {v['family']for v in selected}==set(families) and len(selected)==16,('All fifteen actual classes not acquired',selected)
  if name=='same-kind-properties'and a['phase']in['size','size-again','body']:
   assert len([it for it in window.canvas.selectedItems()if isinstance(it,_SheetItem)])==3
   if a['phase']in['size','size-again']:assert [m.size_pt for m in window.comp.texts]==[float(a['value'])]*2
   else:assert [m.text for m in window.comp.texts]==['Changed']*2
   assert vars(window.comp.images[0])==image_before,'Unrelated selected image changed'
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
 if opaque_model is not None:
  index=window.comp.images.index(opaque_model);opaque[-1]['saved_paths']={'scene.compositions.0.vendor_mixed':opaque[-1]['paths']['scene.compositions.0.vendor_mixed'],'scene.compositions.0.images.'+str(index)+'.vendor_mixed_image':opaque[-1]['paths']['scene.compositions.0.images.0.vendor_mixed_image']}
 active.update(case=name,phase='save');saved=name+'-original.igz';igz.save_scene(scene,out/saved);restored=type(scene)();igz.load_into(restored,out/saved);readers.append(dict(case=name,compositions=[c.to_dict()for c in restored.compositions]));record('save');recipe.update(input=inputfile,saved=saved,actions=executed);(out/'completed-cases.json').write_text(json.dumps(dict(cases=[c for c in cases if 'saved'in c],states=states,readers=readers),ensure_ascii=False,indent=2)+'\n');active.update(case=name,phase='teardown');window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
assert len(errors)==(6 if any(c['name']=='all-kinds-box'for c in cases)else 0),errors
assert all(e['type']=='AttributeError'and e['message']=="'ComposerWindow' object has no attribute '_single_selected'"and'in _on_cota_rad_props'in e['traceback']and e['case']=='all-kinds-box'and e['phase']=='corner-release'for e in errors),errors
report=dict(source_pin=pin,qt=qVersion(),cases=cases,states=states,callback_errors=errors,opaque_guards=opaque,scope='Actual heterogeneous semantic Select/Move/Delete/properties/history/save. Profiles no native painting claim; no clipboard/arrange/resize extension.')
(out/'original-mixed.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(readers,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(cases),states=len(states),readers=len(readers))))
