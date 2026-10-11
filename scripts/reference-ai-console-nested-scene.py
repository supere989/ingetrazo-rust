import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
from core.mesh import Mesh
from core.group import Group,copy_group
from PySide6.QtGui import QMatrix4x4,QVector3D
from formats.igz import save_scene,load_into
seed=Scene();m=Mesh();m.add_face([QVector3D(*p) for p in [(0,0,0),(1,0,0),(1,1,0),(0,1,0)]])
leaf=Group(m,name='Leaf');leaf.xform=QMatrix4x4();leaf.xform.translate(.5,.25,1)
c=Group(name='Child');c.adopt([leaf]);c.xform.rotate(30,0,0,1);c.component=False
p=Group(name='Parent');p.adopt([c]);p.xform.translate(2,3,4);p.component=True
sibling=Group(m,name='Shared');sibling.xform=QMatrix4x4();sibling.xform.scale(2)
p2=copy_group(p);p2.name="SecondParent";p2.xform.translate(-1,0,0);p2.children[0].xform.translate(0,1,0)
seed.groups=[p,sibling,p2];save_scene(seed,Path(sys.argv[1]))
scene=Scene();load_into(scene,Path(sys.argv[1]));history=History(scene);scope={};v=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
def state(name):
 rows=[];meshes=[]
 def walk(g,path,parent):
  mat=g.xform if g.xform is not None else QMatrix4x4(); effective=parent*mat
  rows.append(dict(path=path,name=g.name,matrix=list(mat.data()),effective=list(effective.data()),component=g.is_component(),vertices=[[v.position.x(),v.position.y(),v.position.z()] for v in g.mesh.vertices]));meshes.append(g.mesh)
  for i,c in enumerate(g.children):walk(c,path+[i],effective)
 for i,g in enumerate(scene.groups):walk(g,[i],QMatrix4x4())
 return dict(name=name,rows=rows,aliases=[[m is n for n in meshes]for m in meshes],undo=len(history.undo_stack),redo=len(history.redo_stack))
states=[]
for r in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-nested-scene/recipes.json').read_text()):
 if r['code']:
  result=run_transactional(v,r['code'],scope);assert not result['error'],result
 if r['history'] is not None:assert (history.redo() if r['history'] else history.undo())
 states.append(state(r['name']))
save_scene(scene,Path(sys.argv[1]+'.source-final'));print(json.dumps(dict(states=states),indent=2))
