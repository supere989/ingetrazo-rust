#!/usr/bin/env python3
"""Authenticate focused cached-base/application records without GPU execution."""
import hashlib,json,sys,zipfile
from pathlib import Path
root=Path(sys.argv[1]);manifest=json.loads((root/'manifest.json').read_text())
assert hashlib.sha256((root/'records.zip').read_bytes()).hexdigest()==manifest['archive_sha256']
with zipfile.ZipFile(root/'records.zip')as z:
 assert set(z.namelist())==set(manifest['records'])
 for name,m in manifest['records'].items():
  raw=z.read(name);assert len(raw)==m['bytes'] and hashlib.sha256(raw).hexdigest()==m['sha256']
 for case in ['ordinary','textured']:
  report=json.loads(z.read(case+'/result.json'));rows=report['rows'];assert report['passed'] and len(rows)==16
  worlds=[r['state']['world']for r in rows]
  assert worlds[2]['base_uploads']==worlds[1]['base_uploads']
  assert worlds[10]['base_uploads']==worlds[9]['base_uploads']
  for previous,refused in [(2,3),(5,6),(7,8)]:
   assert rows[previous]['image_sha256']==rows[refused]['image_sha256']
   assert worlds[previous]==worlds[refused]
   assert rows[previous]['state']['committed_bytes']==rows[refused]['state']['committed_bytes']
  if case=='textured':
   for w in worlds[:13]:
    assert w['image_owners']['preview']['decodes']==0 and w['image_owners']['preview']['uploads']==0
   assert worlds[1]['image_owners']['preview']['shared_images']>0
   assert worlds[2]['image_owners']==worlds[1]['image_owners']
  assert b'committed_bytes=0'in z.read(case+'/runtime.log')
 comparison=json.loads(z.read('source-textured/source-comparison.json'))
 assert comparison['extent']==[800,600] and len(comparison['comparisons'])==4
 assert all(r['exact']and r['different_pixels']==0 and r['maximum']==0 for r in comparison['comparisons'])
print('PASS authenticated 32 app actions, cache/refusal/image sharing and four exact source frames')
