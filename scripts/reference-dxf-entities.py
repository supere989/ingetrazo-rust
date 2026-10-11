#!/usr/bin/env python3
"""Only missing straight legacy DXF POLYLINE actual import/context/history."""
import ast,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
import ezdxf
from PySide6.QtWidgets import QApplication,QMainWindow
from views.viewport import Viewport
from core.history import SnapshotImport
from formats import igz,dxf_in
module=ast.parse(Path('scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in ['seed','vec','mesh_state','shape']],type_ignores=[]),'retained-observers','exec'))
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
cases=[];rows=[]
inventory=[('2d-open',False,False,0.,(0,0,1),'root'),('2d-closed',False,True,0.,(0,0,1),'root'),('2d-elevation',False,False,2.,(0,0,1),'root'),('3d-open',True,False,0.,(0,0,1),'root'),('3d-closed',True,True,0.,(0,0,1),'root'),('ocs-negative',False,True,1.,(0,0,-1),'root'),('ocs-oblique',False,False,2.,(1,2,3),'root'),('leaf-3d',True,True,0.,(0,0,1),'leaf')]
for name,is3d,closed,elevation,normal,context in inventory:
 doc=ezdxf.new('R2010');doc.units=6;doc.layers.new('Legacy');msp=doc.modelspace()
 if is3d:entity=msp.add_polyline3d([(0,0,1),(2,0,2),(2,2,3)],dxfattribs={'layer':'Legacy'})
 else:entity=msp.add_polyline2d([(0,0),(2,0),(2,2)],dxfattribs={'layer':'Legacy','elevation':(0,0,elevation),'extrusion':normal})
 entity.close(closed);filename=name+'.dxf';doc.saveas(out/filename)
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;seed(s);igz.save_scene(s,out/(name+'-input.igz'))
 if context=='leaf':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 def record(phase):rows.append({'case':name,'phase':phase,'depth':len(s._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'scene':shape(s)})
 record('before');vp.history.execute(SnapshotImport(lambda scene:dxf_in.load_dxf(scene,out/filename,scale=1.,name=name)));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');vp.end_group_edit();record('finish');igz.save_scene(s,out/(name+'-original.igz'));vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo');cases.append({'name':name,'context':context,'file':filename,'unit':'m'});win.close()
(out/'original-dxf-entities.json').write_text(json.dumps({'cases':cases,'states':rows},indent=2)+'\n');print(json.dumps({'cases':len(cases),'states':len(rows),'actual_source':'dxf_in+SnapshotImport+Viewport'}))
