#!/usr/bin/env python3
"""Missing ezdxf1.4.4 curve law through actual source import/history only."""
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
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
# Frozen straight inputs serve only the unchanged native refusal/Cancel guard.
with zipfile.ZipFile('docs/audits/dxf-legacy-polylines/source-records.zip')as z:
 for n in ['2d-open.dxf','2d-open-input.igz']:(out/n).write_bytes(z.read(n))
rows=[];cases=[];paths=[]
def curves(scene):
 def group(g):return {'edges':len(g.mesh.edges),'curved':sum(e.curve is not None for e in g.mesh.edges),'owners':len({e.curve for e in g.mesh.edges if e.curve is not None}),'children':[group(c)for c in g.children]}
 return [group(g)for g in scene.groups]
inventory=['circle','circle-large','arc-wrap','arc-half','arc-ocs','ellipse-full','ellipse-partial','ellipse-tilted','lw-positive','lw-negative-closed','lw-small-bulge','legacy-bulge-ocs']
for name in inventory:
 doc=ezdxf.new('R2010');doc.units=6;doc.layers.new('Curves');msp=doc.modelspace();attrs={'layer':'Curves'}
 if name.startswith('circle'):entity=msp.add_circle((1,2,0),100. if name=='circle-large'else 1.,dxfattribs=attrs)
 elif name.startswith('arc'):
  start,end=(350,35)if name=='arc-wrap'else(0,180)
  entity=msp.add_arc((1,2,1),2.,start,end,dxfattribs={**attrs,**({'extrusion':(0,0,-1)}if name=='arc-ocs'else{})})
 elif name.startswith('ellipse'):
  major=(2,-1,0)if name=='ellipse-tilted'else(3,1,0);normal=(1,2,3)if name=='ellipse-tilted'else(0,0,1);start,end=(4.7,1.2)if name=='ellipse-partial'else(0,6.283185307179586)
  entity=msp.add_ellipse((1,2,3),major,ratio=.3,start_param=start,end_param=end,dxfattribs={**attrs,'extrusion':normal})
 elif name.startswith('lw'):
  bulge=.5 if name=='lw-positive'else(-1. if name=='lw-negative-closed'else 5e-7)
  entity=msp.add_lwpolyline([(0,0,bulge),(2,0,0),(2,2,.25)],format='xyb',close=name=='lw-negative-closed',dxfattribs=attrs)
 else:
  entity=msp.add_polyline2d([(0,0,.5),(2,0,0),(2,2,-.25)],format='xyb',dxfattribs={**attrs,'elevation':(0,0,2),'extrusion':(1,2,3)});entity.close(True)
 filename=name+'.dxf';doc.saveas(out/filename)
 path=dxf_path.make_path(entity);points=list(path.flattening(.01,segments=8));paths.append({'case':name,'points':[list(V(p.x,p.y,p.z).toTuple())for p in points],'path_vertices':[list(v)for v in path.control_vertices()],'dxf':(out/filename).read_text()})
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;seed(s);igz.save_scene(s,out/(name+'-input.igz'))
 def record(phase):rows.append({'case':name,'phase':phase,'depth':len(s._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'scene':shape(s),'curves':curves(s)})
 record('before');vp.history.execute(SnapshotImport(lambda scene:dxf_in.load_dxf(scene,out/filename,scale=1.,name=name)));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');vp.end_group_edit();record('finish');igz.save_scene(s,out/(name+'-original.igz'));vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo');cases.append({'name':name,'context':'root','file':filename,'unit':'m'});win.close()
(out/'original-dxf-curves.json').write_text(json.dumps({'cases':cases,'states':rows},indent=2)+'\n');(out/'original-dxf-paths.json').write_text(json.dumps(paths,indent=2)+'\n');print(json.dumps({'cases':12,'states':84,'ezdxf':ezdxf.__version__,'actual_source':'dxf_in+SnapshotImport+Viewport'}))
