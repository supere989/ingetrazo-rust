#!/usr/bin/env python3
"""Only prereview-discovered uniform curves/owned-ATTRIB source contracts."""
import ast,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
import ezdxf
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from core.history import SnapshotImport
from formats import igz,dxf_in
for filename,names in [('reference-nested-import.py',['seed','vec','mesh_state','shape']),('reference-dxf-curves.py',['curves']),('reference-dxf-blocks.py',['owners'])]:
 tree=ast.parse(Path('scripts',filename).read_text());exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names],type_ignores=[]),filename,'exec'))
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
assert not(out/'original-dxf-blocks-curve-addendum.json').exists()
rows=[];cases=[]
for name in ['anisotropic-lw-bulge','anisotropic-legacy-bulge','unit-nested-spline']:
 doc=ezdxf.new('R2010');doc.units=6
 inner=doc.blocks.new('Inner')
 if name=='anisotropic-lw-bulge':inner.add_lwpolyline([(0,0,.5),(2,0,0),(2,2,-.25)],format='xyb',close=True,dxfattribs={'elevation':2})
 elif name=='anisotropic-legacy-bulge':
  entity=inner.add_polyline2d([(0,0,.5),(2,0,0),(2,2,-.25)],format='xyb',dxfattribs={'elevation':(0,0,2),'extrusion':(1,2,3)});entity.close(True)
 else:inner.add_rational_spline([(0,0,0),(1,2,.5),(3,-1,1),(4,0,2)],weights=[1,2,.5,1],degree=3,knots=[0,0,0,0,1,1,1,1])
 outer=doc.blocks.new('OuterBlock');outer.add_blockref('Inner',(3,1,2),dxfattribs={'rotation':23,'xscale':-2,'yscale':.5,'zscale':1.5})
 doc.modelspace().add_blockref('OuterBlock',(2,4,0))
 doc.saveas(out/(name+'.dxf'))
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;seed(s);igz.save_scene(s,out/(name+'-input.igz'))
 def record(phase):rows.append(dict(case=name,phase=phase,depth=len(s._edit_stack),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),scene=shape(s),curves=curves(s),owners=owners(s)))
 record('before');vp.history.execute(SnapshotImport(lambda scene:dxf_in.load_dxf(scene,out/(name+'.dxf'),scale=.01 if name=='unit-nested-spline' else 1,name=name)));assert vp.history.last_error is None
 record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');vp.end_group_edit();record('finish');igz.save_scene(s,out/(name+'-original.igz'));vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo')
 cases.append(dict(name=name,context='root',file=name+'.dxf',unit='cm' if name=='unit-nested-spline' else 'm'));win.close()
(out/'original-dxf-blocks-curve-addendum.json').write_text(json.dumps(dict(cases=cases,states=rows),indent=2)+'\n');print(json.dumps(dict(cases=3,states=21,source_pin='6be29fe437a7cd992d4e079f71720821b25216b9')))
