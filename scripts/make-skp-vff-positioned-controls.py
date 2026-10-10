#!/usr/bin/env python3
"""Authored VFF positioned mapping records read by actual pinned OpenSKP."""
import runpy,sys,json,struct
from pathlib import Path
ns=runpy.run_path(str(Path(__file__).with_name('make-skp-vff-texture-controls.py')),run_name='__main__')
tl,geom,file,mat,extras,rows=[ns[n] for n in ['tl','geom','file','mat','extras','rows']]
out=Path(sys.argv[2])
def records(data):
 pos=0
 while pos<len(data):
  tag=data[pos:pos+2].hex().upper();n=struct.unpack_from('<I',data,pos+2)[0];yield tag,data[pos+6:pos+6+n];pos+=6+n
 assert pos==len(data)
def mapping(matrix,side='1127'):
 return tl(side,tl('1327',tl('1527',struct.pack('<9d',*matrix))))
def positioned(data,front,back=None):
 result=b''
 for tag,body in records(data):
  if tag=='AC0D':
   changed=b''
   for t,b in records(body):
    if t=='D007':
     sides=mapping(front)+(mapping(back,'1227')if back is not None else b'')
     b+=tl('DC05',tl('DD05',tl('B136',tl('B236',tl('1027',sides)))))
    changed+=tl(t,b)
   body=changed
  result+=tl(tag,body)
 return result
base=ns['base'];field=base['field'];info=base['info'];draw=base['draw']
identity=[1.,0.,0.,0.,1.,0.,0.,0.,1.]
for name,m in [('positioned-identity',identity),('positioned-rotated',[0.,-1.,0.,1.,0.,0.,4.,7.,1.]),('positioned-perspective',[1.,.125,.01,-.25,1.,.02,3.,4.,1.]),('positioned-singular',[0.]*9)]:
 rows.append(file(name,mat+tl('F601',positioned(geom(material=99,holes=True),m)),extras()))
def backpaint(data,material):
 return b''.join(tl(t,b+field('AF0D',material)if t=='AC0D'else b)for t,b in records(data))
# Back-only painted faces reverse geometry and must select the back matrix.
rows.append(file('positioned-back-only',mat+tl('F601',positioned(backpaint(geom(),99),identity,[0.,-1.,0.,1.,0.,0.,4.,7.,1.])),extras()))
# Different front/back materials retain independently positioned UV recipes.
backmat=tl('C832',info(101)+tl('CC32',b'TileBack'))
assets=extras();assets.update({'materials/TileBack/material.xml':ns['xml']().replace(b'name="Tile"',b'name="TileBack"'),'materials/TileBack/tile.png':ns['png']()})
rows.append(file('positioned-two-sides',mat+backmat+tl('F601',positioned(backpaint(geom(material=99),101),identity,[0.,-1.,0.,1.,0.,0.,4.,7.,1.])),assets))
# Eight tilted neighboring strips share the exact matrix and material.
# Their local-plane UVs disagree at seams; plan XY is continuous.
base=ns['base'];field=base['field'];info=base['info'];draw=base['draw']
def terrain(count):
 data=b''
 for i in range(count):
  x=i*10.;z0=0. if i%2==0 else 10.;z1=10. if i%2==0 else 0.
  points=[(x,0.,z0),(x+10.,0.,z1),(x+10.,10.,z1),(x,10.,z0)]
  vs=[10000+i*4+j for j in range(4)];es=[20000+i*4+j for j in range(4)]
  for v,p in zip(vs,points):data+=tl('C409',info(v)+tl('C509',struct.pack('<3d',*p)))
  for j,e in enumerate(es):data+=tl('B80B',info(e)+field('B90B',vs[j])+field('BA0B',vs[(j+1)%4])+draw())
  loop=tl('9411',b''.join(tl('A00F',field('A10F',e)+field('A20F',0))for e in es))
  data+=tl('AC0D',info(30000+i)+tl('AD0D',struct.pack('<3d',-1. if i%2==0 else 1.,0.,1.))+tl('AE0D',loop)+draw(material=99))
 return positioned(data,identity)
rows.append(file('projected-eight-terrain',mat+tl('F601',terrain(8)),extras()))
rows.append(file('positioned-seven-terrain',mat+tl('F601',terrain(7)),extras()))
manifest=json.loads((out/'manifest.json').read_text());manifest['stage']='positioned-v1';manifest['cases']=rows;(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
