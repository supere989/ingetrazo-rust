#!/usr/bin/env python3
"""Strict actual GPU same-geometry packed-record replacement control."""
from pathlib import Path
import hashlib,json,zipfile,io
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[1];folder=root/'docs/audits/annotation-packed-key';m=json.loads((folder/'archive.json').read_text());raw=(folder/'raw-evidence.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(io.BytesIO(raw))as z:
 assert set(z.namelist())==set(m['members'])
 for n,row in m['members'].items():
  b=z.read(n);assert len(b)==row['bytes'] and hashlib.sha256(b).hexdigest()==row['sha256']
 p='native/ellipse-alpha/'
 images=[np.array(Image.open(io.BytesIO(z.read(p+'packed-key-'+n+'.png'))).convert('RGBA'))for n in ['install','change','restore']]
 states=json.loads(z.read(p+'packed-key-states.json'));log=z.read(p+'runtime.log').decode()
 assert 'NVIDIA' in log and 'Native bounded GPU cleanup complete; present_fences_enabled=true; committed_bytes=0' in log
 ids=[line.split(' action ')[0]for line in log.splitlines()if line.startswith('Packed key same geometry')];assert len(ids)==2 and ids[0]==ids[1]
 assert [s['geometry_uploads']for s in states]==[2,3,4]
 assert len({s['cached_bytes']for s in states})==len({s['committed_bytes']for s in states})==1
 assert all(s['query_uploads']==1 and s['extent']==[800,600]for s in states)
 y,x=np.mgrid[:600,:800];bounds=(x>=100)&(x<200)&(y>=100)&(y<200)
 masks=[bounds&(x+y<299),bounds&(y<=x)]
 def check(im,mask):
  assert im.shape==(600,800,4)
  expected=np.full_like(im,255);expected[mask]=[146,174,206,255];assert np.array_equal(im,expected)
 for im,mask in zip(images[:2],masks):check(im,mask)
 assert np.array_equal(images[0],images[2]) and np.count_nonzero(np.any(images[0]!=images[1],axis=2))==5000
 try:check(images[0],masks[1])
 except AssertionError:pass
 else:raise AssertionError('Stale packed image negative admitted')
print(json.dumps({'same_geometry_id':True,'uploads':[2,3,4],'whole_images_exact':3,'changed_pixels':5000,'stale_image_negative':True,'final_ledger_zero':True}))
