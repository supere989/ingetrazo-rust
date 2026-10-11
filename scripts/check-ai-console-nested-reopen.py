"""Load both real IGZ outputs in pinned original and compare recursive placement/alias state."""
import copy,json,sys
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from formats.igz import load_into
from PySide6.QtGui import QMatrix4x4
import importlib.util
spec=importlib.util.spec_from_file_location('strict',Path(__file__).with_name('check-ai-console-nested-scene.py'));strict=importlib.util.module_from_spec(spec);spec.loader.exec_module(strict);equal=strict.equal

def observe(path):
 s=Scene();load_into(s,Path(path));rows=[];meshes=[]
 def walk(g,path,parent):
  local=g.xform if g.xform is not None else QMatrix4x4();effective=parent*local
  rows.append(dict(path=path,name=g.name,matrix=list(local.data()),effective=list(effective.data()),component=g.is_component(),instance=g.xform is not None,vertices=[[v.position.x(),v.position.y(),v.position.z()]for v in g.mesh.vertices]));meshes.append(g.mesh)
  for i,c in enumerate(g.children):walk(c,path+[i],effective)
 for i,g in enumerate(s.groups):walk(g,[i],QMatrix4x4())
 return dict(rows=rows,aliases=[[a is b for b in meshes]for a in meshes])
a=observe(sys.argv[1]);b=observe(sys.argv[2]);equal(a,b)
for field in ['matrix','effective','vertices']:
 c=copy.deepcopy(b)
 if field=='vertices':c['rows'][2][field][0][2]=99.0
 else:c['rows'][2][field][12]=99.0
 try:equal(a,c)
 except AssertionError:pass
 else:raise AssertionError('altered placement reopened comparison accepted')
print(json.dumps(dict(reopen=a,rejected=3),indent=2))
