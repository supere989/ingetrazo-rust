#!/usr/bin/env python3
"""Only the missing 400/401 source destination boundary; no old source recapture."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.history import History,SnapshotImport
from formats import obj,igz
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);rows=[]
for count in [400,401]:
 path=out/f'threshold-{count}.obj';lines=[]
 for n in range(count):
  x=n*3;lines.extend([f'v {x} 0 0',f'v {x+1} 0 0',f'v {x} 1 0',f'f {n*3+1} {n*3+2} {n*3+3}'])
 path.write_text('\n'.join(lines)+'\n');s=Scene();mesh=Mesh();mesh.add_face([V(0,0,1),V(1,0,1),V(0,1,1)]);g=Group(mesh,'Existing');s.groups.append(g);s.begin_group_edit(g)
 igz.save_scene(s,out/f'threshold-{count}-input.igz');history=History(s)
 def record(phase):rows.append({'case':count,'phase':phase,'active_faces':len(s.mesh.faces),'groups':[{'name':g.name,'faces':len(g.mesh.faces),'children':len(g.children)}for g in s.groups],'undo':len(history.undo_stack),'redo':len(history.redo_stack)})
 record('before');history.execute(SnapshotImport(lambda scene:obj.load_obj(scene,path,scale=1)));record('import');history.undo();record('undo');history.redo();record('redo')
(out/'original-threshold.json').write_text(json.dumps({'states':rows},indent=2)+'\n');print(json.dumps({'cases':2,'states':len(rows),'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9'}))
