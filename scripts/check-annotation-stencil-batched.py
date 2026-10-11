#!/usr/bin/env python3
"""Strict frozen Qt/NVIDIA same-pass stencil prototype, not production adoption."""
import hashlib,io,json,re,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[1]
audit=root/'docs/audits/annotation-stencil-batched'
m=json.loads((audit/'archive.json').read_text())
raw=audit/'raw-evidence.zip'
assert hashlib.sha256(raw.read_bytes()).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(raw) as z,zipfile.ZipFile(root/m['reference_archive']) as source:
 assert set(z.namelist())==set(m['members']) and len(z.namelist())==len(m['members'])
 for n,v in m['members'].items():assert hashlib.sha256(z.read(n)).hexdigest()==v['sha256'] and len(z.read(n))==v['bytes'],n
 oracle=json.loads(source.read('first/source/original.json'))
 names=[f'alpha-{a}-{k}' for a in [0,1,127,128,230,255] for k in ['horizontal','narrow','short','cross']]+['circle','ellipse','ellipse-alpha','ellipse-tiny']
 assert oracle['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name'] for c in oracle['cases']]==names
 result=json.loads(z.read('native/result.json'))
 assert [c['name'] for c in result['rows']]==names
 def image(data):
  a=np.array(Image.open(io.BytesIO(data)).convert('RGBA'));assert a.shape==(600,800,4);return a
 for c,r in zip(oracle['cases'],result['rows']):
  n=c['name'];a=source.read('first/source/'+n+'.png');b=z.read('native/'+n+'/native.png')
  assert hashlib.sha256(a).hexdigest()==c['sha256'] and hashlib.sha256(b).hexdigest()==r['native_sha256']
  assert c['engine']==13 and c['samples']==0 and 'NVIDIA' in c['renderer']
  assert np.array_equal(image(a),image(b)),n
  log=z.read('native/'+n+'/runtime.log').decode()
  assert 'format=S8_UINT extent=800x600 allocation_bytes=532480' in log
  assert 'committed_bytes=0' in log and 'Native Vulkan adapter: "NVIDIA' in log
  final=json.loads(re.search(r'Native annotation metrics: (\{.*\})',log).group(1));state=r['state']
  assert final['geometry_uploads']==state['geometry_uploads'] and final['query_uploads']==state['query_uploads']
  assert final['geometry_cached_bytes']==state['cached_bytes']
  expected=0 if n.startswith('alpha-0-')else (2 if n.endswith('-cross')else 1)if n.startswith('alpha-255-')else 4 if n.endswith('-cross')or n in ['circle','ellipse','ellipse-alpha','ellipse-tiny']else 2
  assert final['draws']==expected*(final['cache_hits']+1),(n,final)
  if n=='ellipse-alpha':
   controls=r['controls'];assert controls['full_frames_unchanged']
   assert [v['extent']for v in controls['resize_states']]==[[704,512],[800,600]]
   assert controls['refusal_state']['committed_bytes']==state['committed_bytes']==controls['resize_states'][-1]['committed_bytes']
   assert controls['refusal_state']['geometry_uploads']==state['geometry_uploads']
   assert 'candidate allocation refused; previous target and ledger retained; restored-budget retry retired'in log
   assert np.array_equal(image(b),image(z.read('native/'+n+'/refusal.png')))
   assert np.array_equal(image(b),image(z.read('native/'+n+'/resize-restored.png')))

 altered=image(b).copy();altered[0,0,0]^=1;assert not np.array_equal(image(b),altered)
 assert names[:-1]!=[r['name'] for r in result['rows']]
 print(json.dumps(dict(exact_whole_images=28,warm_uploads_unchanged=True,final_ledger_zero=True,scope=m['scope']),indent=2))
