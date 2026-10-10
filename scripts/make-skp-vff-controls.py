#!/usr/bin/env python3
"""Author bounded VFF record controls using actual OpenSKP's pinned record grammar.
The original SU_File fixture is preserved; no proprietary blank model or converter.
Generated controls are engineering inputs, not real-user-file era coverage.
"""
import io,zipfile,struct,json,hashlib,sys
from pathlib import Path
source=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
b=source.read_bytes();off=b.find(b'PK\x03\x04');base=zipfile.ZipFile(io.BytesIO(b[off:]));prefix=b[:off]
def tlv(tag,body):return bytes.fromhex(tag)+struct.pack('<I',len(body))+body
def integer(x):return x.to_bytes(max(1,(x.bit_length()+7)//8),'little')
def field(tag,n):return tlv(tag,integer(n))
def info(n):return tlv('DC05',field('DE05',n))
def draw(layer=None,material=None,display=6):
 return tlv('D007',field('D307',display)+(field('D207',layer)if layer else b'')+(field('D107',material)if material else b''))
def geometry(count=1,holes=False,material=None,layer=None):
 data=b'';vi=10;ei=100;fi=1000
 for i in range(count):
  x=i*20.;rings=[[(x,0.,0.),(x+10.,0.,0.),(x+10.,10.,0.),(x,10.,0.)]]
  if holes:rings.append([(x+2.,2.,0.),(x+2.,8.,0.),(x+8.,8.,0.),(x+8.,2.,0.)])
  loops=b''
  for ring in rings:
   vs=list(range(vi,vi+len(ring)));vi+=len(ring);es=list(range(ei,ei+len(ring)));ei+=len(ring)
   for v,p in zip(vs,ring):data+=tlv('C409',info(v)+tlv('C509',struct.pack('<3d',*p)))
   for j,e in enumerate(es):data+=tlv('B80B',info(e)+field('B90B',vs[j])+field('BA0B',vs[(j+1)%len(vs)])+draw(display=30 if j==0 else 6))
   loops+=tlv('9411',b''.join(tlv('A00F',field('A10F',e)+field('A20F',0))for e in es))
  data+=tlv('AC0D',info(fi)+tlv('AD0D',struct.pack('<3d',0,0,1))+tlv('AE0D',loops)+draw(layer,material));fi+=1
 return data
identity=[1.,0.,0.,0.,1.,0.,0.,0.,1.,0.,0.,0.,1.]
def ins(id,x,material=None):
 m=identity.copy();m[9]=x;return tlv('6419',info(8000+id)+field('6719',5000)+tlv('6519',f'placement-{id}'.encode())+tlv('6619',struct.pack('<13d',*m))+draw(material=material))
matxml=b'<materialDocument xmlns="http://sketchup.google.com/schemas/sketchup/1.0/material"><material name="Green" colorRed="12" colorGreen="89" colorBlue="151" trans="0.25" useTrans="1"/></materialDocument>'
mat=tlv('C832',info(99)+tlv('CC32',b'Green'))
layer=tlv('993A',tlv('8C3C',info(2)+tlv('8D3C',b'Hidden tag')+tlv('8E3C',b'\1')))
def file(name,model,extra=None):
 bio=io.BytesIO()
 with zipfile.ZipFile(bio,'w',zipfile.ZIP_DEFLATED)as z:
  def write(n,data):
   info=zipfile.ZipInfo(n,date_time=(2026,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,data)
  for n in base.namelist():
   if n!='model.dat':write(n,base.read(n))
  write('model.dat',tlv('F401',model))
  for n,data in (extra or {}).items():write(n,data)
 p=out/(name+'.skp');p.write_bytes(prefix+bio.getvalue());return {'name':name,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
rows=[]
rows.append(file('hole-paint-layer',mat+layer+tlv('F601',geometry(holes=True,material=99,layer=2)),{'materials/Green/material.xml':matxml,'materials/Layer_Hidden tag/material.xml':matxml.replace(b'name="Green"',b'name="Layer_Hidden tag"')}))
for name,count,copies in [('single-definition',1,1),('small-two-flat',1,2),('shared-eight',64,8)]:
 definition=tlv('7C15',info(5000)+tlv('7E15',b'Leaf')+geometry(count))
 rows.append(file(name,tlv('F901',tlv('7017',tlv('7117',definition)))+tlv('F601',b''.join(ins(i,i*2000.)for i in range(copies)))))
rows.append(file('inherited-paint',mat+tlv('7C15',info(5000)+tlv('7E15',b'Painted')+geometry())+tlv('F601',ins(1,100.,99)),{'materials/Green/material.xml':matxml}))
rows.append(file('texture-refusal',tlv('F601',geometry()),{'materials/Texture/material.xml':b'<materialDocument xmlns="http://sketchup.google.com/schemas/sketchup/1.0/material"><material name="Texture"><texture textureFilename="a.png" xScale="1" yScale="1"/></material></materialDocument>'}))
(out/'manifest.json').write_text(json.dumps({'source_sha256':hashlib.sha256(b).hexdigest(),'openskp_revision':'291700bc213655a00461838de71e5f206311e619','cases':rows},indent=2)+'\n')
