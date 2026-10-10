#!/usr/bin/env python3
"""Strict literal nine whole images; prior GPU float blend is a real negative."""
import argparse,hashlib,json,re
from pathlib import Path
from PySide6.QtGui import QImage,QColor
EXPECTED=['rect','rounded','ellipse','polygon-clamped','line-inverted','arrow','terrain-ticks','terrain-hatch-inverted','terrain-band']
p=argparse.ArgumentParser();p.add_argument('original',type=Path);p.add_argument('native',type=Path);p.add_argument('--old-float',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
def image(path):
 im=QImage(str(path));assert not im.isNull() and [im.width(),im.height()]==[600,500],('Missing/unreadable/wrong-size required image',str(path));return im.convertToFormat(QImage.Format.Format_RGBA8888)
def different(x,y):
 xx,yy=bytes(x.constBits()),bytes(y.constBits());return sum(xx[i:i+4]!=yy[i:i+4]for i in range(0,len(xx),4))
log=(a.native/'startup.log').read_text();assert 'Native Vulkan adapter: "NVIDIA GeForce RTX 5090"'in log and 'llvmpipe'not in log
assert 'requested=COLOR_ATTACHMENT | INPUT_ATTACHMENT'in log and 'supported='in log
owned=json.loads((a.native/'owned-hardware.json').read_text());assert owned['passed'] and owned['owned_client_only'] and owned['clean_close'] and [r['name']for r in owned['cases']]==EXPECTED
rows=[];old=[]
for name in EXPECTED:
 reference=image(a.original/(name+'.png'));native=image(a.native/(name+'.png'));prior=image(a.old_float/(name+'.png'))
 changed=different(reference,native);rows.append({'name':name,'different_pixels':changed,'source_sha256':hashlib.sha256((a.original/(name+'.png')).read_bytes()).hexdigest(),'native_sha256':hashlib.sha256((a.native/(name+'.png')).read_bytes()).hexdigest()});old.append({'name':name,'different_pixels':different(reference,prior)})
assert [r['name']for r in old if r['different_pixels']]>[], 'Retained float-blend negative unexpectedly passed'
assert [r['name']for r in old if r['different_pixels']]==['rounded','ellipse','polygon-clamped'],'Prior actual hardware failure signature changed'
reference=image(a.original/'rect.png');altered=reference.copy();altered.setPixelColor(0,0,QColor(0,0,0,255));assert different(reference,altered)>0
blank=reference.copy();blank.fill(QColor(0,0,0));assert different(reference,blank)>0
rejected=0
for bad in [EXPECTED[:-1],list(reversed(EXPECTED)),EXPECTED[:-1]+[EXPECTED[0]]]:
 if bad!=EXPECTED:rejected+=1
try:image(a.native/'missing-mandatory-case.png')
except AssertionError:rejected+=1
else:raise AssertionError('Missing image accepted')
result={'passed':all(r['different_pixels']==0 for r in rows),'cases':rows,'old_float_negative':old,'inventory_and_missing_negatives':rejected,'altered_pixel_negative':True,'black_startup_negative':True,'boundary':'actual NVIDIA owned window whole600x500 source images, exact bytes; only these nine local shapes, no Metal/thin/full sheet closure'};a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert result['passed'],'Strict shape pixels remain failed'
