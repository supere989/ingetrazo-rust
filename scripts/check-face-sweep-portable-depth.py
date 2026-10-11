#!/usr/bin/env python3
"""Exact retained App/World source images, with portable and native policies."""
import argparse,hashlib,io,json,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
NAMES=['reversed-door','lone-sheet','front-recess','back-recess','front-texture-back-override','ordinary-prism']
PHASES=['base','preview','offset','cancel']
def archive(path):
 path=Path(path);m=json.loads((path/'manifest.json').read_text());raw=(path/'records.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256'];z=zipfile.ZipFile(io.BytesIO(raw));assert len(z.namelist())==len(set(z.namelist()))==len(m['records'])
 for record in m['records']:
  raw=z.read(record['path']);assert len(raw)==record['bytes']and hashlib.sha256(raw).hexdigest()==record['sha256']
 return z,m
def image(raw):
 with Image.open(io.BytesIO(raw))as im:assert im.size==(800,600);return np.array(im.convert('RGBA'))
def compare(a,b):
 assert a.shape==b.shape==(600,800,4);d=np.abs(a.astype(np.int16)-b.astype(np.int16));return int(np.any(d,axis=2).sum()),int(d.max())
def exact(a,b):assert compare(a,b)==(0,0)
p=argparse.ArgumentParser();p.add_argument('audit');p.add_argument('source_audit');args=p.parse_args();z,m=archive(args.audit);s,sm=archive(args.source_audit);proof=json.loads((Path(args.audit)/'provenance.json').read_text());assert proof['source_pin']==sm['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
recipes=json.loads(s.read('complete-config/original.json'))['cases'];assert [r['name']for r in recipes]==NAMES
rows=[];negatives=0
for policy,names in [('portable',NAMES),('native',['reversed-door'])]:
 for name in names:
  t=json.loads(z.read(policy+'/'+name+'/transport.json'));assert t['returncode']==0 and t['source']==proof['runtime_commit']and t['binary_sha256']==proof['binary_sha256']
  env='INGETRAZO_DIAGNOSTIC_PORTABLE_SOURCE_DEPTH=1';assert (env in t['command'])==(policy=='portable')
  result=json.loads(z.read(policy+'/'+name+'/result.json'));assert result['passed']and result['source_sha']==proof['runtime_commit']and result['binary_sha256']==proof['binary_sha256'];assert [r['name']for r in result['rows']]==['acquire','drag','drag','drag','cancel','save'];assert 'committed_bytes=0'in z.read(policy+'/'+name+'/runtime.log').decode()
  recipe=next(r for r in recipes if r['name']==name);assert [f['phase']for f in recipe['frames']]==PHASES
  for i,f in enumerate(recipe['frames'],1):
   raw=s.read('complete-config/'+f['file']);assert hashlib.sha256(raw).hexdigest()==f['sha256'];source=image(raw);native=image(z.read(policy+'/'+name+('/1-acquire.png'if i==1 else '/'+str(i)+'-drag.png')));exact(source,native)
   for kind,corrupted in [('transparent',np.zeros_like(native)),('white',np.full_like(native,255)),('one-byte',native.copy())]:
    if kind=='one-byte':corrupted[0,0,0]^=1
    try:exact(source,corrupted)
    except AssertionError:negatives+=1
    else:raise AssertionError('corrupt image accepted')
   rows.append({'policy':policy,'case':name,'phase':f['phase'],'different_pixels':0,'maximum':0})
# Preserve the real intermediate omission, not merely the final success.
for i,phase,expected in [(2,'preview',(7149,194)),(3,'offset',(7106,194))]:
 assert compare(image(s.read('complete-config/front-texture-back-override-'+phase+'.png')),image(z.read('initial-missing-texture-flag/'+str(i)+'-drag.png')))==expected
assert len(rows)==28 and negatives==84
print(json.dumps({'exact_portable_frames':24,'exact_native_regression_frames':4,'negative_controls':negatives,'rows':rows,'scope':proof['scope']},indent=2))
