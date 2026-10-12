#!/usr/bin/env python3
"""Finite actual Qt legend authoring; unchanged physical painters excluded."""
import argparse,hashlib,json,os,subprocess,sys,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--only',nargs='+');a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/name)
os.environ['QT_QPA_PLATFORM']='offscreen';original=Path(__file__).resolve().parents[1]/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QPushButton
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtGui import QTransform
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,LeyendaItem
from core.composition import Composicion
from core.layers import Layer
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoLegendAuthoringProof');app.setApplicationName('legend-authoring')
os.chdir(out)
def step(phase,op,**kw):return dict(phase=phase,op=op,**kw)
initial_layers=[dict(name='Layer 0'),dict(name='Walls'),dict(name='Hidden',visible=False),dict(name='Locked café',locked=True)]
changed_layers=[dict(name='Layer 0'),dict(name='Hidden'),dict(name='Locked café',visible=False,locked=True),dict(name='New')]
life=[step('initial','observe'),step('armed','arm'),step('press','press',at=[40.,45.]),step('placed','release',at=[40.,45.]),step('title-one','title',value='Planta café'),step('title-two','title',value='Áreas / Walls'),step('title-noop','title',value='Áreas / Walls'),step('layers-changed','layers',value=changed_layers),step('refresh','refresh'),step('refresh-noop','refresh'),step('undo-refresh','undo'),step('redo-refresh','redo'),step('focused-delete','focused-delete'),step('delete','delete'),step('undo-delete','undo'),step('redo-delete','redo'),step('undo-delete-final','undo'),step('select-restored','select',index=0),step('undo-refresh-again','undo'),step('undo-title','undo'),step('undo-add','undo'),step('redo-add','redo'),step('redo-title','redo'),step('redo-refresh-final','redo'),step('save','save')]
empty=[step('initial','observe'),step('armed','arm'),step('cancel-arm','escape'),step('rearmed','arm'),step('pending','press',at=[32.,36.]),step('cancel-pending','escape'),step('release-cancelled','release',at=[32.,36.]),step('cancel-tool','escape'),step('add-arm','arm'),step('add-press','press',at=[33.,37.]),step('empty-placed','release',at=[33.,37.]),step('refresh-empty-noop','refresh'),step('empty-title','title',value=''),step('undo-title','undo'),step('redo-title','redo'),step('save','save')]
loaded=[step('initial','observe'),step('select-locked','select',index=0),step('loaded-title','title',value='Restored snapshot'),step('loaded-refresh','refresh'),step('loaded-delete','delete'),step('undo-loaded-delete','undo'),step('select-again','select',index=0),step('undo-loaded-refresh','undo'),step('undo-loaded-title','undo'),step('redo-loaded-title','redo'),step('redo-loaded-refresh','redo'),step('save','save')]
recipes=[dict(name='lifecycle',layers=initial_layers,composition=dict(name='Legend lifecycle',paper='A4',landscape=True,margin_mm=11.,perfiles=[dict(hidden=True,z=42.)]),actions=life),dict(name='empty-visible-cancel',layers=[dict(name='Layer 0',visible=False),dict(name='Hidden',visible=False,locked=True)],composition=dict(name='Empty legend',paper='A4',landscape=True,margin_mm=10.),actions=empty),dict(name='loaded-locked',layers=initial_layers,composition=dict(name='Loaded legend',paper='A4',landscape=True,margin_mm=10.,leyendas=[dict(x_mm=24.,y_mm=26.,w_mm=67.,title='Old',rows=['Preserved','Old rows'],locked=True,z=9.,list_name='Locked snapshot')]),actions=loaded)]
gesture_actions=[step('initial','observe'),step('move-press','press',at=[50.,45.]),step('move-small','move',at=[54.,48.]),step('move-preview','move',at=[73.,68.]),step('move-release','release',at=[73.,68.]),step('move-undo-source','undo'),step('move-redo-source','redo'),step('resize-press','press',at=[118.,81.5]),step('resize-preview','move',at=[133.,81.5]),step('resize-release','release',at=[133.,81.5]),step('resize-undo-source','undo'),step('resize-redo-source','redo'),step('minimum-press','press',at=[133.,81.5]),step('minimum-preview','move',at=[68.,81.5]),step('minimum-release','release',at=[68.,81.5]),step('save','save')]
recipes.append(dict(name='single-legend-gestures',layers=initial_layers,composition=dict(name='Legend gesture boundary',paper='A4',landscape=True,margin_mm=10.,leyendas=[dict(x_mm=40.,y_mm=40.,w_mm=55.,rows=['One','Two'],list_name='Gesture legend')]),actions=gesture_actions))
if a.only:recipes=[r for r in recipes if r['name']in a.only];assert {r['name']for r in recipes}==set(a.only)
states=[];reopens=[];windows=[];guards=[];callback_errors=[]
def capture_error(t,e,tb):
 import traceback
 callback_errors.append(dict(type=t.__name__,message=str(e),traceback=''.join(traceback.format_exception(t,e,tb))))
 sys.__excepthook__(t,e,tb)
sys.excepthook=capture_error
def flush():app.processEvents();app.processEvents()
for r in recipes:
 name=r['name'];host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene;scene.layers=[Layer.from_dict(v)for v in r['layers']];scene.compositions=[Composicion.from_dict(r['composition'])];inputfile=name+'-input.igz';igz.save_scene(scene,out/inputfile)
 if name=='loaded-locked':
  raw=(out/inputfile).read_bytes();doc=json.loads(raw)if raw[:2]!=b'PK'else json.loads(zipfile.ZipFile(out/inputfile).read('document.json'));c=doc['scene']['compositions'][0];c['vendor_legend_sheet']={'flag':True,'unicode':'café','value':None};c['leyendas'][0]['vendor_legend']={'opaque':[1,'one',None,False]};(out/inputfile).write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n');guards.append(dict(case=name,paths={'scene.compositions.0.vendor_legend_sheet':c['vendor_legend_sheet'],'scene.compositions.0.leyendas.0.vendor_legend':c['leyendas'][0]['vendor_legend']},scope='Original reader drops these raw keys; native preservation is separate.'))
 igz.load_into(scene,out/inputfile)
 window=ComposerWindow(host);windows.append((window,host));window._auto_render=False;window._auto_timer.stop();window.resize(1100,850);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush()
 def selected():
  item=window._selected_item();return next((i for i,m in enumerate(window.comp.leyendas)if item is not None and item.model is m),None)
 def record(phase):states.append(dict(case=name,phase=phase,composition=window.comp.to_dict(),layers=[l.to_dict()for l in scene.layers],selection=selected(),undo=len(window.history._undo),redo=len(window.history._redo),mode=window.tool_mode,pending=view._drag_start is not None,heights=[float(l.h_mm)for l in window.comp.leyendas],source_version=scene.version,callback_error_count=len(callback_errors)))
 for s in r['actions']:
  op=s['op']
  if op=='arm':
   widget=window._tools_tb.widgetForAction(window._tool_actions['leyenda']);assert widget is not None;QTest.mouseClick(widget,Qt.LeftButton)
  elif op in ['press','move','release']:
   pixel=view.mapFromScene(QPointF(*s['at']));pos=view.mapToScene(pixel);s['effective_at']=[pos.x(),pos.y()];s['viewport_pixel']=[pixel.x(),pixel.y()]
   if op=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,Qt.NoModifier,pixel)
   elif op=='release':
    try:QTest.mouseRelease(view.viewport(),Qt.LeftButton,Qt.NoModifier,pixel)
    except AttributeError as error:
     assert name=='single-legend-gestures'and "property 'h_mm'"in str(error),str(error)
     import traceback
     callback_errors.append(dict(case=name,phase=s['phase'],type=type(error).__name__,message=str(error),traceback=traceback.format_exc()))
   else:QTest.mouseMove(view.viewport(),pixel)
  elif op=='escape':QTest.keyClick(view,Qt.Key_Escape)
  elif op=='title':window.ley_title.setFocus();window.ley_title.setText(s['value']);QTest.keyClick(window.ley_title,Qt.Key_Return)
  elif op=='refresh':
   button=next(w for w in window.props.widget(7).findChildren(QPushButton)if w.text()=='Refresh layers');QTest.mouseClick(button,Qt.LeftButton)
  elif op=='layers':scene.layers=[Layer.from_dict(v)for v in s['value']]
  elif op=='focused-delete':
   window.ley_title.setFocus();window.ley_title.end(False);QTest.keyClick(window.ley_title,Qt.Key_Delete)
  elif op=='delete':view.setFocus();QTest.keyClick(view,Qt.Key_Delete)
  elif op=='undo':window._on_undo()
  elif op=='redo':window._on_redo()
  elif op=='select':
   window.canvas.clearSelection();model=window.comp.leyendas[s['index']];next(it for it in window.canvas.items()if isinstance(it,LeyendaItem)and it.model is model).force_select();window.on_selection_changed()
  elif op=='save':
   saved=name+'-original.igz';igz.save_scene(scene,out/saved);restored=type(scene)();igz.load_into(restored,out/saved);reopens.append(dict(case=name,composition=restored.compositions[0].to_dict(),layers=[l.to_dict()for l in restored.layers]));r['saved']=saved
  elif op!='observe':raise AssertionError(op)
  flush();record(s['phase']);(out/'original-legend.partial.json').write_text(json.dumps(dict(cases=recipes,states=states),ensure_ascii=False,indent=2)+'\n')
 r['input']=inputfile;window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
report=dict(source_pin=pin,qt=qVersion(),cases=recipes,states=states,opaque_guards=guards,callback_errors=callback_errors,scope='Actual Qt toolbar/page pointer, title Return, Refresh button, Delete key and original ComposerHistory/writer/reader. Layer replacement is declared fixture publication, not layer editing UI proof. No physical render capture, multi-selection or legend drag/resize claim. Source clocks diagnostic.')
(out/'original-legend.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(reopens,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(recipes),states=len(states),reader_pairs=len(reopens))))
