#!/usr/bin/env python3
"""Only missing OBJ geometry controllers; accepted MTL/nested inputs reused elsewhere."""
import ast,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from views.viewport import Viewport
from core.history import SnapshotImport
from formats import obj,igz
app=QApplication([]);root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
module=ast.parse(Path('scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in ['seed','vec','mesh_state','shape']],type_ignores=[]),'accepted-source-observers','exec'))
def flags(scene):
 def mesh(m):return sorted([{'ends':sorted([vec(e.a),vec(e.b)]),'soft':e.soft}for e in m.edges],key=lambda r:json.dumps(r,sort_keys=True))
 def group(g):return {'mesh':mesh(g.mesh),'children':[group(c)for c in g.children]}
 return {'active':mesh(scene.mesh),'groups':[group(g)for g in scene.groups]}
def observe(vp):
 s=vp.scene;return {'scene':shape(s),'flags':flags(s),'depth':len(s._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack)}
# Closed triangulated cube: every imported face's winding is reversed.
verts=[(0,0,0),(2,0,0),(2,2,0),(0,2,0),(0,0,2),(2,0,2),(2,2,2),(0,2,2)]
quads=[(1,4,3,2),(5,6,7,8),(1,2,6,5),(4,8,7,3),(1,5,8,4),(2,3,7,6)]
base=''.join('v %s %s %s\n'%p for p in verts)
cube=base+''.join('f %s %s %s\n'%tuple(reversed(t))for q in quads for t in [(q[0],q[1],q[2]),(q[0],q[2],q[3])])
# Wedge normals meet at30degrees: explicit groups smooth editable surfaces;
# otherwise reference automatic smoothing alone makes this seam soft.
wedge='v 0 0 0\nv 2 0 0\nv 2 2 0\nv 0 2 0\nv 0 0.2679491924311227 1\nv 2 0.2679491924311227 1\n'
faces='f 1 2 3 4\nf 2 1 5 6\n'
# Multipart always takes reference fusion without welding touching partowners.
part='g Patch\nv 0 0 0\nv 2 0 0\nv 2 2 0\nv 0 2 0\nf 1 2 3\nf 1 3 4\ng Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 5 6 7\n'
# Eight triangles form an annulus; reference fusion retains the actual hole.
annulus='g Ring\n'+''.join('v %s %s 0\n'%p for p in [(0,0),(4,0),(4,4),(0,4),(1,1),(3,1),(3,3),(1,3)])+''.join('f %s %s %s\n'%t for t in [(1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,4,8),(3,8,7),(4,1,5),(4,5,8)])+'g Marker\nv 6 0 0\nv 7 0 0\nv 6 1 0\nf 9 10 11\n'
# Source reference nonmanifold region falls back rather than deleting a triangle.
irregular='g Junction\nv 0 0 0\nv 2 0 0\nv 1 1 0\nv 1 -1 0\nv 1 2 0\nf 1 2 3\nf 2 1 4\nf 1 2 5\ng Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 6 7 8\n'
cases=[('closed-cube-root','root',cube),('closed-cube-leaf','leaf',cube),('explicit-smooth-root','root',wedge+'s 7\n'+faces),('hard-root','root',wedge+faces),('reference-patch','root',part),('reference-annulus','root',annulus),('reference-irregular','root',irregular),('reference-smooth','root','g Wedge\n'+wedge+faces+'g Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 7 8 9\n'),('reference-duplicates','root',part.replace('f 1 2 3\n','f 1 2 3\nf 3 2 1\n'))]
extra=[]
grid=''
for y in range(12):
 for x in range(21):grid+='v %s %s 0\n'%(x,y)
for y in range(11):
 for x in range(20):
  a=y*21+x+1;b=a+1;c=a+22;d=a+21;grid+='f %s %s %s\nf %s %s %s\n'%(a,b,c,a,c,d)
extra.append(('large-reference-grid','root',grid))
extra.append(('reference-auto-smooth','root','g Wedge\n'+wedge.replace('0.2679491924311227 1','2 0.5')+faces+'g Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 7 8 9\n'))
paint='g Paint\nv 0 0 0\nv 4 0 0\nv 3 2 0\nv 0 1 0\nusemtl red\nf 1 2 3\nusemtl blue\nf 1 3 4\ng Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 5 6 7\n'
(root/'fusion-paint.mtl').write_text('newmtl red\nKd 0.2 0.3 0.4\nnewmtl blue\nKd 0.8 0.7 0.6\n')
extra.append(('reference-paint-boundary','root','mtllib fusion-paint.mtl\n'+paint))
if '--addendum'in sys.argv:cases=extra;prior=json.loads((root/'original-obj-fusion.json').read_text());rows=prior['states'];inventory=prior['cases']
else:rows=[];inventory=[]
for name,context,text in cases:
 path=root/(name+'.obj');path.write_text(text);win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp
 if context=='leaf':seed(vp.scene)
 igz.save_scene(vp.scene,root/(name+'-input.igz'))
 if context=='leaf':vp.begin_group_edit(vp.scene.groups[0]);vp.begin_group_edit(vp.scene.edit_group.children[0])
 def record(phase):rows.append({'case':name,'phase':phase,**observe(vp)})
 record('before');vp.history.execute(SnapshotImport(lambda s:obj.load_obj(s,path,scale=1.)));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');vp.end_group_edit();record('finish');igz.save_scene(vp.scene,root/(name+'-original.igz'));inventory.append({'case':name,'context':context,'file':path.name});win.close()
(root/'original-obj-fusion.json').write_text(json.dumps({'cases':inventory,'states':rows},indent=2)+'\n');print(json.dumps({'cases':len(cases),'states':len(rows)}))
