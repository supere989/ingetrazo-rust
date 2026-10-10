#!/usr/bin/env python3
"""Actual pinned VFF grammar nested placement controls, retaining the base corpus."""
import runpy,sys,json,struct
from pathlib import Path
source,out=sys.argv[1:];ns=runpy.run_path(str(Path(__file__).with_name('make-skp-vff-controls.py')),run_name='__main__')
tl,info,field,draw,geom,file=[ns[n]for n in ['tlv','info','field','draw','geometry','file']]
rows=ns['rows'];out=Path(out)
def ins(rid,id,x=0,layer=None,paint=None,m=None):
 matrix=m or [1.,0.,0.,0.,1.,0.,0.,0.,1.,x,0.,0.,1.]
 return tl('6419',info(id)+field('6719',rid)+tl('6519',f'instance-{id}'.encode())+tl('6619',struct.pack('<13d',*matrix))+draw(layer,paint))
def definition(id,name,body):return tl('7C15',info(id)+tl('7E15',name.encode())+body)
def model(defs,root):return tl('F901',tl('7017',tl('7117',b''.join(defs))))+tl('F601',root)
leaf=lambda n:definition(5000,'Leaf',geom(n))
parent=lambda body:definition(6000,'Container',body)
rows.append(file('nested-single-container',model([leaf(1),parent(geom()+ins(5000,10000,20))],ins(6000,20000,200))))
rows.append(file('nested-shared-child',model([leaf(64),parent(geom()+b''.join(ins(5000,10000+i,i*2000)for i in range(8)))],ins(6000,20000,200))))
rows.append(file('nested-shared-parent',model([leaf(32),parent(geom(2)+ins(5000,10000,2000)+ins(5000,10001,4000))],b''.join(ins(6000,20000+i,i*8000)for i in range(8)))))
rows.append(file('nested-proto-in-proto',model([leaf(64),parent(geom()+ins(5000,10000,2000))],b''.join(ins(6000,20000+i,i*8000)for i in range(8)))))
extras={'materials/Layer_Hidden tag/material.xml':ns['matxml'].replace(b'name="Green"',b'name="Layer_Hidden tag"')}
for name,n in [('nested-tagged-flat',1),('nested-tagged-shared',64)]:
 defs=[leaf(n),parent(geom()+b''.join(ins(5000,10000+i,i*2000,layer=2)for i in range(8)))]
 rows.append(file(name,ns['layer']+model(defs,ins(6000,20000,200)+ins(6000,20001,40000)),extras))
# Source inherited painting is keyed per prototype; parent and child paints differ.
bluexml=ns['matxml'].replace(b'name="Green"',b'name="Blue"').replace(b'colorRed="12"',b'colorRed="151"')
blue=tl('C832',info(100)+tl('CC32',b'Blue'))
rows.append(file('nested-inherited-paint',ns['mat']+blue+model([leaf(64),parent(geom()+ins(5000,10000,2000,paint=100))],b''.join(ins(6000,20000+i,i*8000,paint=99)for i in range(8))),{'materials/Green/material.xml':ns['matxml'],'materials/Blue/material.xml':bluexml}))
# Author nonidentity nested transforms in the actual matrix record, not oracle-side manipulation.
m=[-2.,0.,0.,0.,3.,0.,0.,0.,1.,20.,30.,40.,1.]
rows.append(file('nested-reflected-scaled',model([leaf(1),parent(geom()+ins(5000,10000,m=m))],ins(6000,20000,200))))
# Equal paint joins under distinct IDs must fold onto one built prototype.
alias=tl('C832',info(100)+tl('CC32',b'Green'))
rows.append(file('nested-equal-paint-protos',ns['mat']+alias+model([leaf(64)],b''.join(ins(5000,20000+i,i*2000,paint=99 if i%2 else 100)for i in range(8))),{'materials/Green/material.xml':ns['matxml']}))
rot=[0.866025403784,-0.5,0.,0.5,0.866025403784,0.,0.,0.,1.,20.25,30.5,40.75,1.]
outer=[1.5,0.,0.,0.,0.75,0.,0.,0.,2.,200.125,0.25,0.5,1.]
rows.append(file('nested-fractional-transform',model([leaf(1),parent(geom()+ins(5000,10000,m=rot))],ins(6000,20000,m=outer))))
manifest=json.loads((out/'manifest.json').read_text());manifest['stage']='nested-v1';manifest['cases']=rows;(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
