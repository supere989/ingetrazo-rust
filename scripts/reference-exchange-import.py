#!/usr/bin/env python3
"""Actual pinned DAE/GLB chooser/commands/history/save; no original code changes."""
import os,sys,json,ast,struct,base64,traceback,hashlib,subprocess
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN,'original source revision changed'
contract=json.loads(Path('docs/audits/exchange-import/fixture-contract.json').read_text())
for row in contract['original_sources']:
 assert hashlib.sha256((Path('/work/ingetrazo')/row['path']).read_bytes()).hexdigest()==row['sha256'],'original file bytes changed: '+row['path']
for row in contract['sources']:
 assert hashlib.sha256(Path(row['path']).read_bytes()).hexdigest()==row['sha256'],'producer/helper bytes changed: '+row['path']
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
for kind in ['CONFIG','DATA','CACHE']:os.environ['XDG_'+kind+'_HOME']=str(root/('xdg-'+kind.lower()))
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtCore import QSettings,QTimer,Qt
from PySide6.QtTest import QTest
from PySide6.QtGui import QImage,QColor
from views.main_window import MainWindow
from georef.datum import SceneDatum
from formats import igz,dae,glb
import views.main_window as main
from exchange_import_observations import scene_state
app=QApplication([]);app.setOrganizationName('ExchangeImportProof');app.setApplicationName('ExchangeImportProof')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'))
assert not(root/'original-exchange.json').exists(),'frozen output exists'
seedtree=ast.parse(Path('scripts/reference-nested-import.py').read_text());seed_names={'seed'}
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
exec(compile(ast.Module(body=[n for n in seedtree.body if isinstance(n,ast.FunctionDef)and n.name in seed_names],type_ignores=[]),'actual-source-seed','exec'))
_NSDECL='xmlns="http://www.collada.org/2005/11/COLLADASchema"'
tree=ast.parse(Path('/work/ingetrazo/tests/test_dae_import.py').read_text());names={'_cube_dae','_textured_quad_dae'}
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names],type_ignores=[]),'original-dae-fixture-dictionaries','exec'))
folder=root/'assets';folder.mkdir();_cube_dae(folder);(folder/'cube-z.dae').write_bytes((folder/'cube.dae').read_bytes());_cube_dae(folder,'Y_UP','0.0254');(folder/'cube-y.dae').write_bytes((folder/'cube.dae').read_bytes())
textured=_textured_quad_dae(folder)[0].read_text();(folder/'textured.dae').write_text(textured)
card=textured.replace('0 0 0  2 0 0  2 3 0  0 3 0','0 0 0  2 0 0  2 0 3  0 0 3').replace('brick.png','alpha.png')
image=QImage(8,8,QImage.Format_RGBA8888);image.fill(QColor(80,150,30,255));image.setPixelColor(0,0,QColor(80,150,30,0));assert image.save(str(folder/'alpha.png'))
(folder/'card.dae').write_text(card)
# Static cube plus alpha card share one source SnapshotImport but distinct owners.
import xml.etree.ElementTree as ET
ns={'c':'http://www.collada.org/2005/11/COLLADASchema'}
def combined(left,right):
 a=ET.fromstring(left);b=ET.fromstring(right)
 for child in b:
  tag=child.tag.rsplit('}',1)[-1]
  if tag=='asset':continue
  old=next((n for n in a if n.tag==child.tag),None)
  if old is None:a.append(child)
  elif tag=='library_visual_scenes':old[0].extend(list(child[0]))
  else:old.extend(list(child))
 return ET.tostring(a,encoding='unicode')
(folder/'combined.dae').write_text(combined((folder/'cube-z.dae').read_text(),card))
# Same node owns two differently textured geometries: cannot become two sprites.
mixed=ET.fromstring(card);second=ET.fromstring(card.replace('alpha.png','brick.png').replace('img1','img2').replace('fx1','fx2').replace('mat1','mat2').replace('quad','quad2').replace('qp','qp2').replace('quv','quv2').replace('qv','qv2'))
for child in second:
 tag=child.tag.rsplit('}',1)[-1]
 if tag=='asset':continue
 old=next(n for n in mixed if n.tag==child.tag)
 if tag=='library_visual_scenes':old[0][0].extend(list(child[0][0]))
 else:old.extend(list(child))
(folder/'mixed-textures.dae').write_text(ET.tostring(mixed,encoding='unicode'))
def threshold(n):
 pts=' '.join(str(v)for i in range(n)for p in[(i*3,0,0),(i*3+1,0,0),(i*3,1,0)]for v in p)
 return f'<COLLADA {_NSDECL}><asset><up_axis>Z_UP</up_axis></asset><library_geometries><geometry id="g"><mesh><source id="p"><float_array count="{n*9}">{pts}</float_array><technique_common><accessor count="{n*3}" stride="3"/></technique_common></source><vertices id="v"><input semantic="POSITION" source="#p"/></vertices><triangles count="{n}"><input semantic="VERTEX" source="#v"/><p>'+ ' '.join(map(str,range(n*3)))+'</p></triangles></mesh></geometry></library_geometries><library_visual_scenes><visual_scene><node name="Reference"><instance_geometry url="#g"/></node></visual_scene></library_visual_scenes></COLLADA>'
for n in[400,401]:(folder/f'count-{n}.dae').write_text(threshold(n))
geo='<coverage><geographic_location><latitude>51.5</latitude><longitude>-0.1</longitude><altitude>32</altitude></geographic_location></coverage>'
(folder/'datum.dae').write_text((folder/'cube-z.dae').read_text().replace('<up_axis>',geo+'<up_axis>',1));(folder/'failed-datum.dae').write_text(f'<COLLADA {_NSDECL}><asset>{geo}</asset></COLLADA>')
def gltf(name,extent=1,textured=False,binary=True,title=None):
 data=struct.pack('<15f',0,0,0,extent,0,0,0,extent,0,0,0,1,0,0,1)
 doc={'asset':{'version':'2.0'},'buffers':[{'byteLength':len(data)}],'bufferViews':[{'buffer':0,'byteLength':36},{'buffer':0,'byteOffset':36,'byteLength':24}],'accessors':[{'bufferView':0,'componentType':5126,'count':3,'type':'VEC3','min':[0,0,0],'max':[extent,extent,0]},{'bufferView':1,'componentType':5126,'count':3,'type':'VEC2'}],'meshes':[{'primitives':[{'attributes':{'POSITION':0,'TEXCOORD_0':1},'material':0}]}],'materials':[{'pbrMetallicRoughness':{'baseColorFactor':[0.2,0.3,0.4,1]},'alphaMode':'BLEND'}],'nodes':[{'mesh':0}],'scenes':[{'nodes':[0]}],'scene':0}
 if title is not None:doc['asset']['extras']={'title':title}
 if textured:
  doc['images']=[{'uri':'data:image/png;base64,'+base64.b64encode((folder/'alpha.png').read_bytes()).decode()}];doc['textures']=[{'source':0}];doc['materials'][0]['pbrMetallicRoughness']['baseColorTexture']={'index':0,'extensions':{'KHR_texture_transform':{'offset':[.125,.25],'scale':[.5,.75]}}};doc['extensionsUsed']=['KHR_texture_transform']
 if binary:
  raw=json.dumps(doc,separators=(',',':')).encode();raw+=b' '*((-len(raw))%4);body=struct.pack('<I4s',len(raw),b'JSON')+raw+struct.pack('<I4s',len(data),b'BIN\0')+data;(folder/name).write_bytes(b'glTF'+struct.pack('<II',2,12+len(body))+body)
 else:doc['buffers'][0]['uri']='data:application/octet-stream;base64,'+base64.b64encode(data).decode();(folder/name).write_text(json.dumps(doc))
gltf('named.glb',title='界'*70);gltf('textured.glb',textured=True);gltf('scaled.gltf',extent=1000,title='')
cases=[];states=[];errors=[];failure_observations=[];parses=[]
def exception(t,e,tb):errors.append({'type':t.__name__,'message':str(e),'traceback':''.join(traceback.format_exception(t,e,tb))});(root/'callback-errors.json').write_text(json.dumps(errors,indent=2))
sys.excepthook=exception
original_dae=dae.load_dae;original_glb=glb.load_glb
for module in[dae,glb]:
 original=module.load_dae if module is dae else module.load_glb
 def tracked(*a,_original=original,**k):parses.append(str(a[1]));return _original(*a,**k)
 if module is dae:module.load_dae=tracked
 else:module.load_glb=tracked
oldpicker=main.file_dialogs.getOpenFileName
recipes=[('dae-z-root','dae','cube-z.dae','root','accepted'),('dae-y-inch-leaf','dae','cube-y.dae','leaf','accepted'),('dae-400','dae','count-400.dae','root','accepted'),('dae-401-leaf','dae','count-401.dae','leaf','accepted'),('dae-datum','dae','datum.dae','root','accepted'),('dae-existing-datum','dae','datum.dae','root','accepted'),('dae-combined-leaf','dae','combined.dae','leaf','accepted'),('dae-mixed-textures','dae','mixed-textures.dae','root','accepted'),('glb-named','glb','named.glb','root','accepted'),('glb-textured-leaf','glb','textured.glb','leaf','accepted'),('gltf-scale','glb','scaled.gltf','root','accepted'),('chooser-cancel','dae','cube-z.dae','root','cancel'),('dae-failure-datum','dae','failed-datum.dae','root','failed')]
def observe(w,name,phase,outcome):
 states.append({'case':name,'phase':phase,'scene':scene_state(w.viewport.scene),'undo':len(w.viewport.history.undo_stack),'redo':len(w.viewport.history.redo_stack),'depth':len(w.viewport.scene._edit_stack),'outcome':outcome,'camera':{'distance':w.viewport.camera.distance}});(root/'original-exchange.partial.json').write_text(json.dumps({'cases':cases,'states':states,'callback_errors':errors},indent=2))
case_filter=sys.argv[2]if len(sys.argv)>2 else None
assert case_filter is None or case_filter in[c[0]for c in recipes]
for name,kind,file,context,outcome in recipes:
 if case_filter is not None and name!=case_filter:continue
 w=MainWindow();w.show();app.processEvents();seed(w.viewport.scene)
 if name=='dae-existing-datum':w.viewport.scene.georef=SceneDatum(lat=10,lon=20,alt=7,north=12)
 igz.save_scene(w.viewport.scene,root/(name+'-input.igz'))
 if context=='leaf':w.viewport.begin_group_edit(w.viewport.scene.groups[0]);w.viewport.begin_group_edit(w.viewport.scene.edit_group.children[0])
 if name=='dae-combined-leaf':
  from core.history import SnapshotImport
  w.viewport.history.execute(SnapshotImport(lambda scene:scene.mesh.add_edge(V(20,1,0),V(21,1,0))))
 before=scene_state(w.viewport.scene);history=(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack));camera=w.viewport.camera.distance;count=len(parses);observe(w,name,'before','idle')
 main.file_dialogs.getOpenFileName=lambda *a,**k:('','')if outcome=='cancel'else(str(folder/file),kind)
 (w._on_import_dae if kind=='dae'else w._on_import_glb)();app.processEvents();observe(w,name,'import',outcome)
 assert w.viewport.camera.distance==camera,'import changed camera'
 saved=None
 if outcome=='accepted':
  assert scene_state(w.viewport.scene)!=before and len(w.viewport.history.undo_stack)==history[0]+1
  assert len(parses)==count+1,'parse-once actual command'
  if name=='dae-combined-leaf':assert len(w.viewport.scene.mesh.faces)>len(before['active']['faces'])and any(getattr(g,'billboard',None)=='mesh'for g in w.viewport.scene.groups),'combined owners missing'
  if name=='dae-mixed-textures':assert not any(getattr(g,'billboard',None)for g in w.viewport.scene.groups),'mixed texture node classified as sprite'
  w.viewport.history.undo();observe(w,name,'undo','accepted');restored=scene_state(w.viewport.scene)
  expected=dict(before)
  if name=='dae-datum':expected['datum']=states[-2]['scene']['datum']
  assert restored==expected,'Undo ownership changed'
  w.viewport.history.redo();observe(w,name,'redo','accepted')
  while w.viewport.scene._edit_stack:w.viewport.end_group_edit()
  observe(w,name,'finish','accepted')
  if name=='dae-combined-leaf':
   finished=scene_state(w.viewport.scene);w.viewport.history.undo();observe(w,name,'finish-undo','accepted');assert scene_state(w.viewport.scene)!=finished;w.viewport.history.redo();observe(w,name,'finish-redo','accepted');assert scene_state(w.viewport.scene)==finished
  saved=name+'-saved.igz';igz.save_scene(w.viewport.scene,root/saved)
 elif outcome=='cancel':assert scene_state(w.viewport.scene)==before and history==(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack)) and len(parses)==count
 else:
  last_error=w.viewport.history.last_error
  assert last_error=='SnapshotImport: No geometry found in the COLLADA file',last_error
  failure_observations.append({'case':name,'last_error':last_error,'status':w.statusBar().currentMessage(),'title':w.windowTitle(),'import_name':str(w._import_name),'source_ui_fault':'History.execute swallows failure; MainWindow continues Imported status/title without critical dialog','native_correction':'atomic refusal and actual error UI'})
  (root/'source-failure-observation.json').write_text(json.dumps(failure_observations,indent=2)+'\n')
  assert history==(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack));after=scene_state(w.viewport.scene);assert after['datum']is not None and before['datum']is None,'source adopted-datum fault not witnessed';after['datum']=None;assert after==before
 cases.append({'name':name,'kind':kind,'file':'assets/'+file,'input':name+'-input.igz','context':context,'outcome':outcome,'saved':saved,'phases':[s['phase']for s in states if s['case']==name]});(root/'completed-cases.json').write_text(json.dumps({'cases':cases,'states':states},indent=2));w.hide();w.deleteLater();app.processEvents()
main.file_dialogs.getOpenFileName=oldpicker;dae.load_dae=original_dae;glb.load_glb=original_glb;assert not errors,errors
(root/'original-exchange.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':cases,'states':states,'callback_errors':errors,'failure_observations_diagnostic':failure_observations,'capture_case_filter':case_filter,'source_faults':{'dae-failure-datum':'datum adoption survives geometry failure; swallowed error allows source Imported status/title; native atomic refusal and error UI separately declared'},'scope':'Actual App command/chooser-provider/history/save. No unit/options dialog or camera fit. Asset unit/up-axis, rooted reference groups, FACEME, combined nested owners. Source failure datum leakage retained; native correction is explicit. No viewport pixels or broader format/version coverage.'},indent=2)+'\n');print(json.dumps({'cases':len(cases),'states':len(states)}))
