"""Actual pinned Python loader validates native shared prototype publication."""
import sys
from pathlib import Path
sys.path.insert(0, '/work/ingetrazo')
from core.scene import Scene
from formats.igz import load_into
s=Scene(); load_into(s,Path(sys.argv[1]))
def validate(s):
 a,b=s.groups[0],s.groups[-1]
 assert a.mesh is b.mesh
 assert a.mesh.faces[0].attrs['opacity']==0.625
 assert a.mesh.faces[0].attrs['color']==[0.9,0.87,0.8,1.0]
validate(s)
for kind in ['alias','opacity']:
 t=Scene();load_into(t,Path(sys.argv[1]))
 if kind=='alias':
  from core.mesh import Mesh
  t.groups[-1].mesh=Mesh()
 else:t.groups[0].mesh.faces[0].attrs['opacity']=0.5
 try:validate(t)
 except AssertionError:pass
 else:raise AssertionError('accepted '+kind)
print('actual original native IGZ shared Mesh identity/current paint +2 negatives PASS')
u=Scene();load_into(u,Path(sys.argv[1]+'.unpublished'))
try:validate(u)
except AssertionError:pass
else:raise AssertionError('accepted literal unpublished registry')
print('literal old absent-registry persisted alias failure rejected')
