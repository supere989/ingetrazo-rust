#!/usr/bin/env python3
"""Strict revision-specific actual Qt/RTX alpha-union prototype evidence."""
import hashlib,io,json,re,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[1]
audit=root/'docs/audits/annotation-alpha-union'
manifest=json.loads((audit/'archive.json').read_text());archive=audit/'raw-evidence.zip'
assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['sha256']
with zipfile.ZipFile(archive)as z:
 assert set(z.namelist())==set(manifest['members'])and len(z.namelist())==len(manifest['members'])
 for name,digest in manifest['members'].items():assert hashlib.sha256(z.read(name)).hexdigest()==digest,name
 def data(name):return json.loads(z.read(name))
 def image(name):
  a=np.array(Image.open(io.BytesIO(z.read(name))).convert('RGBA'));assert a.shape==(600,800,4);return a
 def count(a,b):return int(np.any(a!=b,axis=2).sum())
 names=[f'alpha-{a}-{kind}'for a in [0,1,127,128,230,255]for kind in ['horizontal','narrow','short','cross']]
 oracle=data('first/source/original.json');assert oracle['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['name']for r in oracle['cases']]==names+['circle','ellipse','ellipse-alpha','ellipse-tiny']
 result=data('matched-lines/native/result.json');assert [r['name']for r in result['rows']]==names
 first=[]
 for row in oracle['cases']:
  name=row['name'];assert row['engine']==13 and row['samples']==0 and row['framebuffer']==[800,600]and'NVIDIA'in row['renderer']
  raw=z.read('first/source/'+name+'.png');assert hashlib.sha256(raw).hexdigest()==row['sha256']
  a=image('first/source/'+name+'.png');old=image('first/native/'+name+'/native.png');first.append(count(a,old))
  if name not in names:continue
  b=image('matched-lines/native/'+name+'/native.png');assert count(a,b)==0,name
  state=data('matched-lines/native/'+name+'/state.json');assert state['pixels_per_point']==1.
  log=z.read('matched-lines/native/'+name+'/runtime.log').decode();assert'committed_bytes=0'in log and'Native Vulkan adapter: "NVIDIA'in log
  final=json.loads(re.search(r'Native annotation metrics: (\{.*\})',log).group(1))
  assert final['geometry_uploads']==state['geometry_uploads']and final['query_uploads']==state['query_uploads']and final['geometry_cached_bytes']==state['cached_bytes']
 assert first==[0]*4+[0,0,8,60]*4+[0]*4+[179,190,528,3],first
 mutated=b.copy();mutated[0,0,0]^=1;assert count(b,mutated)==1
 assert names[:-1]!=[r['name']for r in result['rows']]
 print(json.dumps(dict(exact_alpha_lines=24,old_composite_bounds_failure_pixels=[8,60],retained_ellipse_failures=[179,190,528,3],warm_uploads_unchanged=True,final_ledger_zero=True,scope='probe-only; throughput and full curve/application parity open'),indent=2))
