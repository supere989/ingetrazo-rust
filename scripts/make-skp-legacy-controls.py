#!/usr/bin/env python3
"""Classic MFC controls on the actual pinned single-material-v17 archive prefix.
Uses documented pinned legacy.py readers and real capilla v17 class schemas;
these authored controls do not prove real-user legacy geometry coverage.
"""
from pathlib import Path
import sys,struct,json,hashlib
source=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True);base=source.read_bytes()
assert len(base)==5858
# Actual pinned _walk ends its zero-root count at byte3577, store slots9..12.
root_count_at=3573
assert base[root_count_at:root_count_at+4]==b'\0'*4
schemas={'CFace':3,'CLoop':1,'CEdgeUse':1,'CEdge':2,'CVertex':0}
def make(name,holes=False,count=1,hidden=False,reverse=False,null_entry=False):
 classes={'CMaterial':9,'CLayer':11};next_slot=13;vertices={};edge_ids=[]
 def obj(name,body):
  nonlocal next_slot
  if name not in classes:
   classes[name]=next_slot;next_slot+=1;encoded=struct.pack('<HHH',65535,schemas[name],len(name))+name.encode()
  else:encoded=struct.pack('<H',32768|classes[name])
  slot=next_slot;next_slot+=1;return slot,encoded+body(slot)
 def ref(slot):return struct.pack('<H',slot)
 def pre():return b'\0\0\0' # null attrs and v17 zero PID mask
 def draw(material=0,invisible=False):return struct.pack('<H6BH',material,int(invisible),0,0,0,0,0,0)
 def vertex(point):
  if point in vertices:return ref(vertices[point])
  slot,encoded=obj('CVertex',lambda _:pre()+struct.pack('<3d',*point));vertices[point]=slot;return encoded
 def edge(a,b):
  slot,encoded=obj('CEdge',lambda _:pre()+draw()+vertex(a)+vertex(b)+b'\0\0');edge_ids.append(slot);return encoded
 def loop(points):
  def body(parent):
   data=pre()+b'\0\0'
   for i,a in enumerate(points):
    _,encoded=obj('CEdgeUse',lambda _:pre()+(edge(points[(i+1)%len(points)],a)if reverse else edge(a,points[(i+1)%len(points)]))+bytes([int(reverse)])+ref(parent));data+=encoded
   return data+b'\0\0'
  return obj('CLoop',body)[1]
 def face(i):
  x=i*20.;outer=[(x,0.,0.),(x+10.,0.,0.),(x+10.,10.,0.),(x,10.,0.)];rings=[outer]
  if holes:rings.append([(x+2.,2.,0.),(x+2.,8.,0.),(x+8.,8.,0.),(x+8.,2.,0.)])
  return obj('CFace',lambda _:pre()+draw(10,hidden and i==1)+struct.pack('<4dI',0.,0.,1.,0.,len(rings))+b''.join(loop(r)for r in rings)+b'\0\0')[1]
 bodies=b''.join(face(i)for i in range(count));body=bodies+b''.join(ref(e)for e in edge_ids)
 data=base[:root_count_at]+struct.pack('<I',count+len(edge_ids)+int(null_entry))+body+(b'\0\0'if null_entry else b'')+base[root_count_at+4:]
 path=out/(name+'.skp');path.write_bytes(data);return{'name':name,'sha256':hashlib.sha256(data).hexdigest()}
rows=[make('legacy-v17-painted-quad'),make('legacy-v17-painted-hole',holes=True),make('legacy-v17-two-faces',count=2),make('legacy-v17-hidden-face',count=2,hidden=True),make('legacy-v17-reversed-coedges',reverse=True),make('legacy-v17-null-list-item',null_entry=True)]
(out/'manifest.json').write_text(json.dumps({'source_sha256':hashlib.sha256(base).hexdigest(),'openskp_revision':'291700bc213655a00461838de71e5f206311e619','cases':rows},indent=2)+'\n')
