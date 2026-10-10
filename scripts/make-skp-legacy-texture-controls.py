#!/usr/bin/env python3
"""Authored MFC texture controls on the pinned real v17 header/layer prefix."""
from pathlib import Path
import sys,struct,json,hashlib
from PySide6.QtCore import QBuffer,QIODevice
from PySide6.QtGui import QImage
source,out=map(Path,sys.argv[1:3]);out.mkdir(parents=True,exist_ok=True);original=source.read_bytes()
assert len(original)==5858
producer=Path(__file__).with_name('make-skp-legacy-controls.py').read_text();producer=producer[producer.index("schemas="):producer.index("rows=")]
producer=producer.replace("'CLayer':11", "'CLayer':13").replace('next_slot=13', 'next_slot=15')
def text(s):
 b=s.encode('utf-16-le');assert len(s)<255;return b'\xff\xfe\xff'+bytes([len(s)])+b
def write(name,png,target,flag,w,h,tile_w,tile_h,transparency,use_op,mat_name,filename):
 material=struct.pack('<HHH',65535,12,9)+b'CMaterial'+b'\0\0\0'+text(mat_name)+struct.pack('<H',1)+b'\0\0'
 material+=struct.pack('<HHH',65535,0,4)+b'CDib'+struct.pack('<II',0,len(png))+png
 avg=bytes([*target,255 if flag else 0]);material+=struct.pack('<2d',tile_w,tile_h)+text(filename)+avg+b'\0'+avg+text('')+struct.pack('<II',0,int(flag))+struct.pack('<dB',transparency,use_op)
 prefix=original[:3400]+material+original[3462:3567]+struct.pack('<HI',14,0)
 base=prefix+struct.pack('<I',0)+original[3577:];root_count_at=len(prefix)
 ns={'struct':struct,'base':base,'root_count_at':root_count_at,'Path':Path,'out':out,'hashlib':hashlib};exec(producer,ns)
 row=ns['make'](name);row.update(width=w,height=h,target=target,colorized=flag,material_name=mat_name,png_sha256=hashlib.sha256(png).hexdigest());return row
rows=[]
cases=[
 ('legacy-dib-unflagged',16,16,(20,40,60),False,False),
 ('legacy-dib-greedy-average',16,16,(20,40,60),True,False),
 ('legacy-dib-tint-green',16,16,(12,173,69),True,False),
 ('legacy-dib-tint-alpha',16,16,(181,61,19),True,True),
 ('legacy-dib-tint-odd',3,5,(110,11,203),True,False),
 ('legacy-dib-guard-small',3,5,(35,55,75),True,False),
 ('legacy-dib-guard-large',3,5,(90,100,120),True,False),
 ('legacy-dib-fully-transparent',16,16,(181,61,19),True,True),
 ('legacy-dib-empty',16,16,(20,40,60),True,False)]
real=[]
if len(sys.argv)>3:
 oracle=Path(sys.argv[3]);sys.path.insert(0,str(oracle/'packages/python/src'))
 from openskp import legacy
 for model in ['capilla_quiroz_v17.skp','legacy_v7_synthetic.skp']:
  file=oracle/'packages/python/tests/fixtures'/model;ar,_,_,mats=legacy._walk(file.read_bytes())
  for slot,m in mats:
   if 'tex_dib'in m:real.append((f'legacy-real-{model.split("_")[0]}-{slot}',model,slot,m,ar.slots[m['tex_dib']][2]['data'],hashlib.sha256(file.read_bytes()).hexdigest()))
for name,w,h,target,flag,transparent in cases:
 solid='greedy' in name or 'guard' in name or 'unflagged' in name
 px=bytes(c for y in range(h)for x in range(w)for c in ((20,40,60,255)if solid else ((x*17)%256,(y*31)%256,((x+y)*13)%256,0 if 'fully-transparent' in name else (0 if transparent and x%3==0 else 128 if transparent else 255))))
 img=QImage(px,w,h,w*4,QImage.Format_RGBA8888);buf=QBuffer();buf.open(QIODevice.WriteOnly);assert img.save(buf,'PNG');png=bytes(buf.data())
 if name=='legacy-dib-empty':png=b''
 row=write(name,png,target,flag,w,h,24.,12.,.25,1,'RedMat','source.png');rows.append(row)
for name,model,slot,m,data,sha in real:
 image=QImage.fromData(data);assert not image.isNull()
 row=write(name,data,m['rgba'][:3],m.get('colorized',False),image.width(),image.height(),m['tex_w'],m['tex_h'],m['opacity'],m['use_opacity'],m['name'],m['tex_file']);row.update(real_model=model,real_material_slot=slot,real_model_sha256=sha);rows.append(row)
img=QImage(16,16,QImage.Format_RGBA8888);img.fill(0xff112233);buf=QBuffer();buf.open(QIODevice.WriteOnly);assert img.save(buf,'PNG');raw=bytes(buf.data());corrupt=write('legacy-dib-truncated',raw[:raw.index(b'IDAT')+6],(20,40,60),False,w,h,24.,12.,.25,1,'RedMat','corrupt.png')
(out/'manifest.json').write_text(json.dumps({'source_sha256':hashlib.sha256(original).hexdigest(),'openskp_revision':'291700bc213655a00461838de71e5f206311e619','cases':rows,'integrity_cases':[corrupt]},indent=2)+'\n')
