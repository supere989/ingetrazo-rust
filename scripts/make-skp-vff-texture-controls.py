#!/usr/bin/env python3
"""Bounded actual VFF archive image/default-local-UV controls; no renderer mock."""
import runpy,sys,json,struct,zlib
from pathlib import Path
source,out=sys.argv[1:];ns=runpy.run_path(str(Path(__file__).with_name('make-skp-vff-nested-controls.py')),run_name='__main__')
base=ns['ns'];tl,info,field,geom,file=[base[n]for n in ['tlv','info','field','geometry','file']];rows=ns['rows'];out=Path(out)
def png():
 def chunk(k,b):return struct.pack('>I',len(b))+k+b+struct.pack('>I',zlib.crc32(k+b))
 # Two rows of two RGBA pixels, straight authored bytes.
 scan=b'\0'+bytes([10,20,180,64,10,50,180,104])+b'\0'+bytes([50,20,180,64,50,50,180,104])
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,2,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b'')
mat=tl('C832',info(99)+tl('CC32',b'Tile'))
def xml(w=10,h=20,filename='tile.png'):
 return f'<materialDocument xmlns="http://sketchup.google.com/schemas/sketchup/1.0/material"><material name="Tile" colorRed="12" colorGreen="89" colorBlue="151" trans="0.25" useTrans="1"><texture textureFilename="{filename}" xScale="{w}" yScale="{h}"/></material></materialDocument>'.encode()
extras=lambda w=10,h=20,filename='tile.png':{'materials/Tile/material.xml':xml(w,h,filename),'materials/Tile/tile.png':png()}
rows.append(file('texture-default-hole',mat+tl('F601',geom(holes=True,material=99)),extras()))
rows.append(file('texture-sentinel-size',mat+tl('F601',geom(material=99)),extras(1,1e-231)))
rows.append(file('texture-default-size',mat+tl('F601',geom(material=99)),extras(0,0)))
rows.append(file('texture-sibling-fallback',mat+tl('F601',geom(material=99)),extras(filename='other-name.png')))
# Paint whole nested/shared components; world placement must not change their local patch.
leaf=ns['leaf'];parent=ns['parent'];ins=ns['ins'];model=ns['model']
rows.append(file('texture-shared-local',mat+model([leaf(64)],b''.join(ins(5000,20000+i,i*2000,paint=99)for i in range(8))),extras()))
rot=[0.866025403784,-0.5,0.,0.5,0.866025403784,0.,0.,0.,1.,20.25,30.5,40.75,1.]
rows.append(file('texture-nested-transform',mat+model([leaf(1),parent(ins(5000,10000,m=rot))],ins(6000,20000,200,paint=99)),extras()))
# Default UV uses file local normal; downward normal selects (-X,+Y).
body=geom(material=99).replace(tl('AD0D',struct.pack('<3d',0,0,1)),tl('AD0D',struct.pack('<3d',0,0,-1)))
rows.append(file('texture-downward-basis',mat+tl('F601',body),extras()))
manifest=json.loads((out/'manifest.json').read_text());manifest['stage']='textures-v1';manifest['cases']=rows;(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
