#!/usr/bin/env python3
"""Read-only replay of compact cold-resource timings and actual GPU controls."""
import copy, hashlib, io, json, pathlib, statistics, zipfile
import numpy as np
from PIL import Image
ROOT=pathlib.Path(__file__).resolve().parents[1]
AUDIT=ROOT/'docs/audits/texture-cold-performance'
def sha(b): return hashlib.sha256(b).hexdigest()
def pixels(b):
 im=Image.open(io.BytesIO(b)).convert('RGBA'); assert im.size==(800,600); return np.array(im)
def admit(j):
 assert len(j['rows'])==4
 assert [(r['set'],r['iteration'],r['driver_warmup'])for r in j['rows']]==[('small',0,True),('small',1,False),('large',0,True),('large',1,False)]
 for r in j['rows']:
  assert r['cold_set_scene_ms']>0 and r['warm_extra_submissions']==0 and r['ledger_final']==0
  assert r['phases']['image_uploads']==5 and r['phases']['bc7_uploads']==1
  assert r['resources']['image_mips_generated']==(26 if r['set']=='small' else 40)
  assert r['resources']['staging_peak']<=8*1024*1024
  assert r['resources']['image_bytes']==(315456 if r['set']=='small' else 8388672)
 assert len(j['large_encoded_blake3'])==4
m=json.loads((AUDIT/'manifest.json').read_text());blob=(AUDIT/'captures.zip').read_bytes();assert sha(blob)==m['zip_sha256']
with zipfile.ZipFile(io.BytesIO(blob))as z:
 assert set(z.namelist())=={r['name']for r in m['records']}
 for r in m['records']:
  b=z.read(r['name']);assert len(b)==r['bytes'] and sha(b)==r['sha256']
 results={}; hashes={}; encoded=None
 for phase in ['batching-negative','final']:
  rows={}; sources={}; bins={}
  files=sorted(n for n in z.namelist()if n.startswith(phase+'/')and n.endswith('/perf.json'));assert len(files)==10
  for n in files:
   j=json.loads(z.read(n));admit(j);variant=n.split('/')[1].split('-')[1]
   sources.setdefault(variant,j['source']);assert sources[variant]==j['source']
   t=json.loads(z.read(n.replace('perf.json','transport.json')));assert t['returncode']==0
   bins.setdefault(variant,t['binary_sha256']);assert bins[variant]==t['binary_sha256']==m['binaries'][variant]['sha256']
   assert 'NVIDIA GeForce RTX 5090' in z.read(n.replace('perf.json','runtime.log')).decode()
   if encoded is None:encoded=j['large_encoded_blake3']
   assert encoded==j['large_encoded_blake3']
   for r in j['rows']:
    hashes.setdefault(r['set'],r['pixel_blake3']);assert hashes[r['set']]==r['pixel_blake3']
    assert r['resources']['transfer_submissions']==((7 if r['set']=='small' else 8)if variant=='baseline' else (3 if r['set']=='small' else 4))
    if not r['driver_warmup']: rows.setdefault((variant,r['set']),[]).append(r['cold_set_scene_ms'])
  summary={}
  for (variant,case),values in rows.items():
   assert len(values)==5;summary[variant+'_'+case]={'samples_ms':values,'median_ms':statistics.median(values),'range_ms':[min(values),max(values)]}
  results[phase]={'summary':summary,'sources':sources,'binaries':bins}
 reference=(ROOT/m['reference_zip']).read_bytes();assert sha(reference)==m['reference_zip_sha256']
 assert m['frozen_original_app']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 with zipfile.ZipFile(io.BytesIO(reference))as source:
  for case in ['near','far']:
   a=pixels(source.read('original/'+case+'.png'));b=pixels(z.read('controls/00-controls/'+case+'.png'));assert np.array_equal(a,b),case
   changed=b.copy();changed[10,10,0]^=1;assert not np.array_equal(a,changed)
 c=json.loads(z.read('controls/00-controls/native.json'));assert c['old_frame_retained'] and c['retry_exact'] and c['ledger_final']==0
 assert c['warm']['transfer_submissions']==c['cold']['transfer_submissions']
 l=c['large'];assert l['all_base_bytes_exact'] and l['warm_uploads']==0 and l['ledger_final']==0 and l['completed_retirement']==1
 assert l['raw_bytes']==17825792 and l['cold']['image_transfer_submissions']==3 and l['cold']['staging_peak']<=8*1024*1024
 bad=copy.deepcopy(j);bad['rows'].pop()
 try:admit(bad)
 except AssertionError:pass
 else:raise AssertionError('Missing measured row admitted')
 bad=copy.deepcopy(j);bad['rows'][0]['resources']['image_mips_generated']=0
 try:admit(bad)
 except AssertionError:pass
 else:raise AssertionError('Mutated mip accounting admitted')
 results['controls']={'source':c['source'],'near_far_full_rgba_exact':True,'budget_refusal_retry_exact':True,'large17MiB_exact':True,'ledger_final':0,'negative_controls':4}
 print(json.dumps(results,indent=2))
