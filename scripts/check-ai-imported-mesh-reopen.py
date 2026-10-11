"""Operative pinned loader on both source and native persisted alias outputs."""
import sys,json
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from formats.igz import load_into

def observe(path):
 s=Scene();load_into(s,Path(path));return s

def validate(s):
 assert len(s.groups)==2
 a,b=s.groups
 assert a.mesh is b.mesh
 assert a.is_component() and b.is_component()
 assert a.xform.isIdentity() and b.xform.isIdentity()
 assert a.mesh.faces[0].attrs['opacity']==0.375
 assert a.mesh.faces[0].attrs['color']==[0.2,0.4,0.6,1.0]
 assert a.mesh.vertices[0].position.z()==0.25
source=observe(sys.argv[1]);native=observe(sys.argv[2]);validate(source);validate(native)
for kind in ['alias','component','position','paint']:
 s=observe(sys.argv[2])
 if kind=='alias':
  from core.mesh import Mesh
  s.groups[1].mesh=Mesh()
 elif kind=='component':s.groups[0].component=False
 elif kind=='position':s.groups[0].mesh.vertices[0].position.setZ(0.5)
 else:s.groups[0].mesh.faces[0].attrs['opacity']=0.5
 try:validate(s)
 except AssertionError:pass
 else:raise AssertionError('accepted '+kind)
print('actual source/native IGZ aliases/component/placement/current payload +4 negatives PASS')
def validate_replacement(s):
 assert len(s.groups)==3
 a,b=s.groups[:2]
 assert a.mesh is not b.mesh
 assert a.is_component() and b.is_component()
 assert a.xform.isIdentity() and b.xform.isIdentity()
 assert len(a.mesh.vertices)==3 and len(b.mesh.vertices)==4
 assert a.mesh.faces[0].attrs['opacity']==0.75
 assert b.mesh.faces[0].attrs['opacity']==0.625
validate_replacement(observe(sys.argv[1].removesuffix('.source-final')+'.source-replacement'))
validate_replacement(observe(sys.argv[2]+'.replacement'))
s=observe(sys.argv[2]+'.replacement');s.groups[0].mesh=s.groups[1].mesh
try:validate_replacement(s)
except AssertionError:pass
else:raise AssertionError('accepted stale replacement prototype')
print('actual source/native replacement independent prototypes/current payload +old-token negative PASS')
