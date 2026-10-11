#!/usr/bin/env python3
"""Strict whole-frame camera/depth correction and retained causal controls."""
import argparse,hashlib,json,zipfile,io
from pathlib import Path
import numpy as np
from PIL import Image
NAMES=['reversed-door','lone-sheet','front-recess','back-recess','front-texture-back-override','ordinary-prism']
PHASES=['base','preview','offset','cancel']
def archive(path):
 path=Path(path);m=json.loads((path/'manifest.json').read_text());raw=(path/'records.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256'];z=zipfile.ZipFile(io.BytesIO(raw));assert len(z.namelist())==len(set(z.namelist()))==len(m['records'])
 for r in m['records']:
  b=z.read(r['path']);assert len(b)==r['bytes']and hashlib.sha256(b).hexdigest()==r['sha256']
 return z,m
def image(raw):
 with Image.open(io.BytesIO(raw))as im:assert im.size==(800,600);return np.array(im.convert('RGBA'))
def compare(a,b):
 assert a.shape==b.shape==(600,800,4);d=np.abs(a.astype(np.int16)-b.astype(np.int16));return int(np.any(d,axis=2).sum()),int(d.max())
def exact(a,b):assert compare(a,b)==(0,0)
a=argparse.ArgumentParser();a.add_argument('audit');a.add_argument('source_audit');args=a.parse_args();z,m=archive(args.audit);s,sm=archive(args.source_audit);assert sm['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
p=json.loads((Path(args.audit)/'provenance.json').read_text());recipes=json.loads(s.read('complete-config/original.json'))['cases'];assert [r['name']for r in recipes]==NAMES
rows=[];negatives=0
for recipe in recipes:
 name=recipe['name'];assert [f['phase']for f in recipe['frames']]==PHASES
 t=json.loads(z.read('final/'+name+'/transport.json'));assert t['returncode']==0 and t['source']==p['runtime_commit'] and t['binary_sha256']==p['application_binary_sha256']
 result=json.loads(z.read('final/'+name+'/result.json'));assert result['passed']and result['source_sha']==p['runtime_commit']and result['binary_sha256']==p['application_binary_sha256'];assert [r['name']for r in result['rows']]==['acquire','drag','drag','drag','cancel','save']
 assert 'committed_bytes=0'in z.read('final/'+name+'/runtime.log').decode()
 for i,f in enumerate(recipe['frames'],1):
  raw=s.read('complete-config/'+f['file']);assert hashlib.sha256(raw).hexdigest()==f['sha256'];source=image(raw);native=image(z.read('final/'+name+('/1-acquire.png'if i==1 else '/'+str(i)+'-drag.png')));exact(source,native)
  for kind,corrupted in [('transparent-blank',np.zeros_like(native)),('opaque-white-blank',np.full_like(native,255)),('one-byte',native.copy())]:
   if kind=='one-byte':corrupted[0,0,0]^=1
   try:exact(source,corrupted)
   except AssertionError:negatives+=1
   else:raise AssertionError('blank or one-byte corruption accepted')
  rows.append({'case':name,'phase':f['phase'],'different_pixels':0,'maximum':0})
causal=[]
for mode in ['ordinary','qt-camera','qt-portable']:
 for phase in PHASES:
  source=image(s.read('complete-config/reversed-door-'+phase+'-gl-only.png'));native=image(z.read('diagnostic/'+mode+'/reversed-door-'+phase+'.png'));actual=compare(source,native);expected=(539,81)if phase=='preview'and mode!='qt-camera'else(1079,122)if phase=='offset'and mode!='qt-camera'else(0,0);assert actual==expected
  causal.append({'mode':mode,'phase':phase,'different_pixels':actual[0],'maximum':actual[1]})
print(json.dumps({'source_parity':True,'exact_frames':len(rows),'negative_controls':negatives,'rows':rows,'causal':causal,'scope':'RTX5090 actual App/World/label complete frames; portable fallback remains a strict failure'},indent=2))
