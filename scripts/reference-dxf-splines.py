#!/usr/bin/env python3
"""Only missing source SPLINE construction/path workflows, no accepted recapture."""
import ast,json,os,sys,zipfile
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
import ezdxf
from ezdxf import path as dxf_path
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.history import SnapshotImport
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from formats import igz,dxf_in
module=ast.parse(Path('scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in ['seed','vec','mesh_state','shape']],type_ignores=[]),'retained-observers','exec'))
module=ast.parse(Path('scripts/reference-dxf-curves.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name=='curves'],type_ignores=[]),'retained-curve-observer','exec'))
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile('docs/audits/dxf-legacy-polylines/source-records.zip')as z:
 for n in ['2d-open.dxf','2d-open-input.igz']:(out/n).write_bytes(z.read(n))
rows=[];cases=[];paths=[]
names=['cubic-single','cubic-spans','knots-rounded','quadratic','quartic','rational-quadratic','rational-cubic','periodic','periodic-rational','knots-shifted','missing-knots','fit-natural','fit-tangents','closed-flag-open-path','invalid-weights-with-line']
base=[(0,0,0),(1,2,1),(2,-1,2),(3,1,0),(4,3,1),(5,-2,0),(6,0,2)]
for name in names:
 doc=ezdxf.new('R2010');doc.units=6;doc.layers.new('Splines');msp=doc.modelspace();attrs={'layer':'Splines'}
 if name.startswith('fit-'):
  e=msp.add_spline(fit_points=base[:5],dxfattribs=attrs)
  if name=='fit-tangents':e.dxf.start_tangent=(1,1,0);e.dxf.end_tangent=(1,-1,1)
 elif name.startswith('periodic'):
  e=msp.add_spline(dxfattribs=attrs)
  if name=='periodic':e.set_closed(base[:5],degree=3)
  else:e.set_closed_rational(base[:5],[1.,.5,2.,1.,.75],degree=3)
 elif name.startswith('rational'):
  e=msp.add_spline(dxfattribs=attrs)
  if name=='rational-quadratic':e.set_open_rational([(1,0,0),(1,1,0),(0,1,0)],[1.,2**-.5,1.],degree=2)
  else:e.set_open_rational(base[:4],[1.,.5,2.,1.],degree=3)
 elif name=='quadratic':e=msp.add_open_spline(base[:5],degree=2,dxfattribs=attrs)
 elif name=='quartic':e=msp.add_open_spline(base,degree=4,dxfattribs=attrs)
 elif name in ['cubic-spans','knots-rounded']:
  e=msp.add_open_spline(base,dxfattribs=attrs)
  if name=='knots-rounded':e.knots=[0,0,0,0,.300000000004,.599999999996,.8,1,1,1,1]
 else:
  e=msp.add_open_spline(base[:4],dxfattribs=attrs)
  if name=='knots-shifted':e.knots=[2,2,2,2,4,4,4,4]
  elif name=='missing-knots':e.knots=[]
  elif name=='closed-flag-open-path':e.dxf.flags=1
  elif name=='invalid-weights-with-line':e.weights=[1.];msp.add_line((10,0,0),(11,1,0),dxfattribs=attrs)
 filename=name+'.dxf';doc.saveas(out/filename)
 # Read the saved DXF through the same library as the actual loader, including
 # repairs/default attributes, before recording the emitted path law.
 loaded=dxf_in.open_document(out/filename);emitted=[]
 for entity in loaded.modelspace():
  runs,curved=dxf_in._entity_segments(entity,.01)
  if runs:
   for run in runs:emitted.append({'points':[list(V(p.x,p.y,p.z).toTuple())for p in run],'curved':curved})
 paths.append({'case':name,'dxf':(out/filename).read_text(),'entities':emitted})
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;seed(s);igz.save_scene(s,out/(name+'-input.igz'))
 context='leaf'if name=='periodic-rational'else'root'
 if context=='leaf':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 def record(phase):rows.append({'case':name,'phase':phase,'depth':len(s._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'scene':shape(s),'curves':curves(s)})
 record('before');vp.history.execute(SnapshotImport(lambda scene:dxf_in.load_dxf(scene,out/filename,scale=1.,name=name)));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');vp.end_group_edit();record('finish');igz.save_scene(s,out/(name+'-original.igz'));vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo');cases.append({'name':name,'context':context,'file':filename,'unit':'m'});win.close()
# Source invalid-only entity raises before group/layer publication.
doc=ezdxf.new('R2010');doc.units=6;e=doc.modelspace().add_open_spline(base[:4]);e.weights=[1.];doc.saveas(out/'invalid-only.dxf')
win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);s=vp.scene;seed(s)
before=shape(s);h=(len(vp.history.undo_stack),len(vp.history.redo_stack))
vp.history.execute(SnapshotImport(lambda scene:dxf_in.load_dxf(scene,out/'invalid-only.dxf',scale=1.)))
assert vp.history.last_error is not None
assert before==shape(s);assert h==(len(vp.history.undo_stack),len(vp.history.redo_stack));win.close()
(out/'original-dxf-splines.json').write_text(json.dumps({'cases':cases,'states':rows},indent=2)+'\n');(out/'original-dxf-spline-paths.json').write_text(json.dumps(paths,indent=2)+'\n');(out/'original-spline-errors.json').write_text(json.dumps({'invalid_only_preserves_document_history':True},indent=2)+'\n');print(json.dumps({'cases':len(cases),'states':len(rows),'ezdxf':ezdxf.__version__,'invalid_only_preserves_document_history':True}))
