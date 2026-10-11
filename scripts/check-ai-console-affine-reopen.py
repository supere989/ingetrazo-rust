import copy,json,sys
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from formats.igz import load_into
def read(path):
 scene=Scene();load_into(scene,Path(path))
 return dict(names=[g.name for g in scene.groups],transforms=[list(g.xform.data()) if g.xform else None for g in scene.groups],aliases=[[g.mesh is h.mesh for h in scene.groups] for g in scene.groups],components=[g.is_component() for g in scene.groups])
def check(a,b):
 assert a==b
 assert len(a['names'])==3 and all(a['components']) and all(all(row) for row in a['aliases'])
a,b=map(read,sys.argv[1:]);check(a,b)
for key in ['transforms','aliases','components']:
 c=copy.deepcopy(b)
 if key=='transforms':c[key][0][12]+=.125
 elif key=='aliases':c[key][0][1]=False
 else:c[key][0]=False
 try:check(a,c)
 except AssertionError:pass
 else:raise AssertionError('negative accepted '+key)
print(json.dumps(dict(reopen=a,negative_controls=3),indent=2))
