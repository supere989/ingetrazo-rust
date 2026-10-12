#!/usr/bin/env python3
"""Actual unchanged composer anchor synchronization and model overlay tuple producer."""
import argparse,json,os,sys,subprocess,traceback,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/name)
os.environ['QT_QPA_PLATFORM']='offscreen';repo=Path(__file__).resolve().parents[1];original=repo/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow,QToolButton,QComboBox,QCheckBox
from PySide6.QtGui import QVector3D,QTransform
from PySide6.QtCore import Qt,QPointF,qVersion
from PySide6.QtTest import QTest
from views.viewport import Viewport
from views.composer import ComposerWindow,FrameItem
from core.composition import Composicion
from core.dimension import Dimension
from core.textlabel import TextLabel
from core.layers import Layer
from core.saved_views import SavedView
from core.section import SectionPlane
from core.history import MoveVerticesCommand
from georef.geopath import GeoPath
from formats import igz
app=QApplication([]);app.setOrganizationName('IngeTrazoFrameAnnotationProof');app.setApplicationName('frame-annotations');os.chdir(out);contract=json.loads((repo/'docs/audits/frame-annotation-sync/fixture-contract.json').read_text());states=[];cases=contract['cases'];readers=[];errors=[];active={'case':None,'phase':'init'};windows=[]
def fail(t,e,tb):
 errors.append(dict(**active,type=t.__name__,message=str(e),traceback=''.join(traceback.format_exception(t,e,tb))));(out/'source-errors.json').write_text(json.dumps(errors,indent=2)+'\n');sys.__excepthook__(t,e,tb)
sys.excepthook=fail
from frame_annotation_observations import xyz,plain,geometry,annotations,viewport_state
def flush():app.processEvents();app.processEvents()
for recipe in cases:
 name=recipe['name'];active.update(case=name,phase='init');host=QMainWindow();host.viewport=Viewport();vp=host.viewport;s=vp.scene;width=recipe.get('fixture_requirements',{}).get('model_width_m',12. if name=='section-traces'else 1.);points=[QVector3D(0,0,0),QVector3D(width,0,0),QVector3D(width,1,0),QVector3D(0,1,0)];s.mesh.add_face([points[0],points[1],points[2]]);s.mesh.add_face([points[0],points[2],points[3]]);s.layers=[Layer('Layer 0'),Layer('Model annotations'),Layer('Hidden annotations',visible=False)];s.dimension_style={'ends':'arrow','units':'m','decimals':2}
 s.dimensions=[Dimension(points[0],points[1],QVector3D(0,-.2,0),layer='Model annotations',text='Length <>'),Dimension(points[0],points[2],QVector3D(.1,0,0),layer='Hidden annotations',axis='x')]
 for dim in s.dimensions:dim.bind(s)
 s.text_labels=[TextLabel(points[2],QVector3D(.15,.1,0),'Café\nLine two',layer='Model annotations'),TextLabel(points[0],QVector3D(-.2,-.1,0),'Left side',layer='Model annotations'),TextLabel(points[1],QVector3D(.1,0,0),'Hidden',layer='Hidden annotations')]
 frame=dict(uid='main',x_mm=20.,y_mm=25.,w_mm=100.,h_mm=70.,scale_n=recipe.get('fixture_requirements',{}).get('scale_n',100.),view_key='std:top',style='vectorial',show_title=False,annotations=name!='model-dimensions-and-text',cam_distance=4.)
 comp=dict(name=name,paper='A4',landscape=True,margin_mm=10.,frames=[frame])
 if 'anchored_sheet_sync'in recipe['paths']:
  comp.update(cotas=[dict(x_mm=30.,y_mm=35.,dx_mm=10.,dy_mm=0.,anchor_uid='main',a_world=[0.,0.,0.],b_world=[width,0.,0.],scale_n=100.,sep_mm=8.),dict(x_mm=180.,y_mm=140.,dx_mm=15.,anchor_uid='missing-frame',a_world=[0.,0.,0.],b_world=[width,0.,0.])],etiquetas=[dict(x_mm=150.,y_mm=65.,text='Fixed text',anchor_uid='main',a_world=[width,0.,0.],leaders=[dict(ax_mm=-20.,ay_mm=-10.,anchor_uid='main',a_world=[width,0.,0.])])],niveles=[dict(x_mm=70.,y_mm=100.,ax_mm=8.,ay_mm=6.,anchor_uid='main',a_world=[width,0.,0.],datum_m=.01,text='Height {z}')])
 if name=='paths-and-chainage':s.geo_paths=[GeoPath([QVector3D(0,0,0),QVector3D(.75,0,0),QVector3D(.75,.5,0)],name='Road'),GeoPath([QVector3D(.1,.1,0),QVector3D(.4,.1,0),QVector3D(.4,.4,0)],closed=True,name='Closed')]
 if name=='section-traces':s.section_planes=[SectionPlane(QVector3D(width/2,.5,0),QVector3D(1,0,0),symbol='B',uid=101),SectionPlane(QVector3D(.5,.5,0),QVector3D(0,0,1),symbol='Face-on',uid=102),SectionPlane(QVector3D(width+3,0,0),QVector3D(1,0,0),symbol='Miss',uid=103),SectionPlane(QVector3D(width/2,.5,0),QVector3D(0,1,0),symbol='Width clip',uid=104)]
 if name=='saved-view-and-perspective':s.text_labels.append(TextLabel(QVector3D(.5,.5,0),QVector3D(0,-10,0),'Behind eye'));s.saved_views=[SavedView('Hidden layer detail',target=(.5,.5,0),distance=4.,yaw=-math.pi/2,pitch=.35,perspective=False,hidden_layers=['Model annotations'],section={'active':None,'planes':False,'cuts':False})]
 s._bounds_cache=None;s.compositions=[Composicion.from_dict(comp)];inputfile=name+'-input.igz';igz.save_scene(s,out/inputfile);window=ComposerWindow(host);windows.append((window,host));window._auto_render=False;window._auto_timer.stop();window.resize(1100,850);window.show();flush();view=window._view;view.setTransform(QTransform().scale(2.,2.));flush()
 def frame_by_uid(uid):return next(f for f in window.comp.frames if f.uid==uid)
 def select_frame(uid):
  f=frame_by_uid(uid);window.canvas.clearSelection();next(it for it in window.canvas.items()if isinstance(it,FrameItem)and it.model is f).force_select();window.on_selection_changed();flush();return f
 def toolbar(action):
  button=next(w for w in window.findChildren(QToolButton)if w.defaultAction()is action);QTest.mouseClick(button,Qt.LeftButton);flush()
 def pointer(op,at):
  pixel=view.mapFromScene(QPointF(*at));actual=view.mapToScene(pixel)
  if op=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,Qt.NoModifier,pixel)
  elif op=='release':QTest.mouseRelease(view.viewport(),Qt.LeftButton,Qt.NoModifier,pixel)
  else:QTest.mouseMove(view.viewport(),pixel)
  flush();return[actual.x(),actual.y()]
 def record(phase):
  before=viewport_state(vp);composition=window.comp.to_dict();overlay={};snaps={}
  for f in window.comp.frames:
   overlay[f.uid]=plain(window.compute_annotations(f));page,world=window.frame_snap_points(f);snaps[f.uid]=dict(page=page.tolist(),world=world.tolist())
  after=viewport_state(vp);assert before==after,('Frame observation leaked viewport state',name,phase,before,after)
  states.append(json.loads(json.dumps(dict(case=name,phase=phase,composition=composition,model_geometry=geometry(s),model_annotations=annotations(s),overlay_by_frame=overlay,snap_by_frame=snaps,sheet_undo=len(window.history._undo),sheet_redo=len(window.history._redo),model_undo=len(vp.history.undo_stack),model_redo=len(vp.history.redo_stack),viewport_restored=True,source_errors=len(errors)),allow_nan=False)));(out/'original-frame-annotations.partial.json').write_text(json.dumps(dict(cases=cases,states=states),ensure_ascii=False,indent=2)+'\n')
 for action in recipe['actions']:
  active.update(case=name,phase=action['phase']);op=action['op']
  if op=='frame_properties':
   select_frame(action['frame']);mapping={'scale_n':'scale_combo','rot_deg':'rot_spin','view_key':'view_combo','w_mm':'fw_spin','h_mm':'fh_spin','annotations':'annot_check','annot_text_mm':'annot_mm_spin','km_marks':'km_check','km_step_m':'km_step_spin','section_marks':'secmark_check','perspective':'persp_check','cam_fov':'fov_spin'}
   for key,value in action['fields'].items():
    widget=getattr(window,mapping[key]);assert widget.isEnabled(),key
    if isinstance(widget,QCheckBox):widget.setChecked(value)
    elif isinstance(widget,QComboBox):
     if key=='scale_n':widget.setCurrentText('1:'+str(value))
     else:index=widget.findData(value);assert index>=0,(key,value);widget.setCurrentIndex(index)
    else:widget.setValue(value)
    flush()
  elif op=='frame_move':
   toolbar(window._tool_actions['select']);f=select_frame(action['frame']);item=next(it for it in window.canvas.items()if isinstance(it,FrameItem)and it.model is f);start=[f.x_mm+10.,f.y_mm+10.];pixel=view.mapFromScene(QPointF(*start));effective=view.mapToScene(pixel);local=item.mapFromScene(effective)
   assert item.boundingRect().contains(local)and item.shape().contains(local)and not item._on_resize_handle(local),('Frame body admission',name)
   assert next((it for it in view.items(pixel)if isinstance(it,FrameItem)),None)is item,('Frame body hit ownership',name)
   before_xy=(f.x_mm,f.y_mm);before_undo=len(window.history._undo);end=[start[i]+action['delta_mm'][i]for i in range(2)];action['pointer_actions']=[dict(op=x,effective_at=pointer(x,p))for x,p in [('press',start),('move',end),('release',end)]]
   assert(f.x_mm,f.y_mm)!=before_xy and len(window.history._undo)==before_undo+1,('Frame drag was not operative',name,before_xy,(f.x_mm,f.y_mm),before_undo,len(window.history._undo))
  elif op=='create_anchored_cota':
   before_count=len(window.comp.cotas);before_undo=len(window.history._undo);f=frame_by_uid(action['frame']);page=window._frame_world_to_page(f,action['world_points']);toolbar(window._tool_actions['cota']);mid=[(page[0][i]+page[1][i])/2 for i in range(2)];dx=page[1][0]-page[0][0];dy=page[1][1]-page[0][1];length=math.hypot(dx,dy);third=[mid[0]-dy/length*action['offset_mm'],mid[1]+dx/length*action['offset_mm']];action['pointer_actions']=[]
   for point in [page[0],page[1],third]:
    pointer('move',point);action['pointer_actions'] +=[dict(op=x,effective_at=pointer(x,point))for x in ['press','release']]
   assert len(window.comp.cotas)==before_count+1 and len(window.history._undo)==before_undo+1,('Cota creation was not operative',name)
   action['acquired']=window.comp.to_dict()['cotas'][-1];toolbar(window._tool_actions['select'])
  elif op=='model_move':
   source=[QVector3D(s.mesh.vertices[i].position)for i in action['vertices']];vp.history.execute(MoveVerticesCommand(source,QVector3D(*action['delta_m'])));assert vp.history.last_error is None;window._on_model_version(s.version)
  elif op in ['model_undo','model_redo']:
   getattr(vp.history,op[6:])();window._on_model_version(s.version)
  elif op in ['sheet_undo','sheet_redo']:getattr(window,'_on_'+op[6:])()
  elif op=='frame_delete':select_frame(action['frame']);view.setFocus();QTest.keyClick(view,Qt.Key_Delete)
  elif op=='fixture_style':s.dimension_style.update(action['style'])
  elif op=='refresh':
   assert not any(f.perspective for f in window.comp.frames),'Full GL Update is outside offscreen semantic capture';toolbar(window.refresh_action)
  elif op=='save':
   saved=name+'-original.igz';igz.save_scene(s,out/saved);restored=type(s)();igz.load_into(restored,out/saved);readers.append(dict(case=name,composition=restored.compositions[0].to_dict(),model_geometry=geometry(restored),model_annotations=annotations(restored)));recipe['saved']=saved
  elif op!='observe':raise AssertionError(op)
  flush();record(action['phase'])
 recipe['input']=inputfile;active.update(case=name,phase='teardown');window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();flush()
assert not errors,errors
report=dict(source_pin=pin,qt=qVersion(),cases=cases,states=states,source_errors=errors,scope='Actual frame UI/model history/derived synchronization and original model overlay tuples; perspective observation is semantic only, no GL/physical pixel claim. Fixture layer/style initialization explicitly declared.')
(out/'original-frame-annotations.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(readers,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(cases),states=len(states),readers=len(readers))))
