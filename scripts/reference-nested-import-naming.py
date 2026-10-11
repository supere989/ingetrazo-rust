#!/usr/bin/env python3
"""Only loaded/repeated default naming and original piece cleanup boundaries."""
import os,sys,json,shutil
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.history import History,SnapshotImport
from core.i18n import set_language
from formats import obj,igz
app=QApplication([]);set_language('en');out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
mesh=Mesh();mesh.add_face([V(0,0,1),V(1,0,1),V(0,1,1)]);s=Scene();s.groups.append(Group(mesh,'Group 50'));igz.save_scene(s,out/'naming-input.igz')
s=Scene();igz.load_into(s,out/'naming-input.igz');history=History(s);rows=[]
shutil.copyfile(sys.argv[2],out/'naming-large.obj')
def record(phase):rows.append({'phase':phase,'names':[g.name for g in s.groups],'undo':len(history.undo_stack),'redo':len(history.redo_stack)})
record('before')
for phase in ['first','second']:
 history.execute(SnapshotImport(lambda scene:obj.load_obj(scene,out/'naming-large.obj',scale=1)));record(phase)
history.undo();record('undo');history.redo();record('redo')
names=['1_Cube.001','Door.001_Cube.007','','.001','A_cube','Base-unit-Box-Right_Cube.002']
(out/'original-naming.json').write_text(json.dumps({'states':rows,'piece_names':[{'authored':name,'result':obj._piece_name(('',name),1)}for name in names]},indent=2)+'\n');print(json.dumps({'states':5,'piece_names':6,'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9'}))
