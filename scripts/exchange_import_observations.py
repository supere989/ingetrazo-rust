"""Independent original geometry observation; resource locators use actual bytes."""
import json,hashlib
from pathlib import Path
from PySide6.QtGui import QMatrix4x4 as M

def vec(v):return list(v.toTuple())
def texture(t):
 t=dict(t)
 if 'path' in t:
  path=Path(t.pop('path'));t['resource']={'encoded_sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
 return t

def mesh_state(mesh,m=None):
 def point(p):return vec(m.map(p)if m is not None else p)
 def ring(v):
  p=[point(x)for x in v];return min(p[i:]+p[:i]for i in range(len(p)))if p else[]
 def attrs(f):
  a=dict(f.attrs)
  if isinstance(a.get('texture'),dict):a['texture']=texture(a['texture'])
  return a
 return {'points':sorted(point(v.position)for v in mesh.vertices),'edges':sorted([{'points':sorted([point(e.a),point(e.b)]),'soft':bool(e.soft),'hidden':bool(e.hidden)}for e in mesh.edges],key=lambda x:json.dumps(x,sort_keys=True)),'faces':sorted([{'outer':ring(f.vertices),'holes':sorted(ring(h)for h in f.holes),'attrs':attrs(f)}for f in mesh.faces],key=lambda f:json.dumps(f,sort_keys=True))}

def scene_state(s):
 def group(g,parent):
  matrix=parent*(g.xform if g.xform is not None else M())
  return {'name':g.name,'layer':g.layer,'local_matrix':[float(x)for x in g.xform.data()]if g.xform is not None else None,'billboard':getattr(g,'billboard',None),'mesh':mesh_state(g.mesh,matrix),'children':[group(c,matrix)for c in g.children]}
 return {'active':mesh_state(s.mesh),'loose':mesh_state(s.loose_mesh),'groups':[group(g,M())for g in s.groups],'layers':[l.name for l in s.layers],'units':s.units,'datum':s.georef.to_dict()if s.georef is not None else None}
