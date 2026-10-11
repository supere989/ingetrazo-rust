#!/usr/bin/env python3
"""Authenticate and strictly compare the frozen native Text3D corpus."""
import copy,hashlib,json,math,sys,zipfile
from pathlib import Path
NAMES=['holes','concave','kerning','descenders','spaces','flat','large','bold','italic','bold-italic']
audit=Path(__file__).resolve().parents[1]/'docs/audits/text3d-font'
m=json.loads((audit/'manifest.json').read_text());raw=(audit/'records.zip').read_bytes()
assert hashlib.sha256(raw).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(audit/'records.zip') as z:
 assert set(z.namelist())==set(m['records'])
 for name,record in m['records'].items():
  data=z.read(name);assert len(data)==record['bytes'] and hashlib.sha256(data).hexdigest()==record['sha256'],name
 source=json.loads(z.read('original.json'));native=json.loads(z.read('native.json'));old=json.loads(z.read('negative-flattening.json'))
assert source['source']=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert source['qt']=='6.12.0' and source['dpi']==96.0
assert [x['name'] for x in source['cases']]==NAMES
assert all(x['font']['pixel_size']==133 for x in source['cases'])
def check(rows):
 assert isinstance(rows,list) and [x['name'] for x in rows]==NAMES
 for a,b in zip(source['cases'],rows):
  assert a['meshes']==b['meshes'],a['name']+' ordered mesh mismatch'
  assert a['fingerprint']==b['state']['text3d']['hash'],a['name']+' fingerprint mismatch'
check(native)
failures=[a['name'] for a,b in zip(source['cases'],old) if a['meshes']!=b['meshes']]
assert failures==['holes','kerning','descenders','flat','large','bold','italic','bold-italic']
try:check(old)
except AssertionError:pass
else:raise AssertionError('compiled non-source flattening admitted')
for i in range(len(native)):
 for kind in ('coordinate','soft','empty'):
  bad=copy.deepcopy(native)
  if kind=='coordinate':bad[i]['meshes'][0]['vertices'][0][0]=math.nextafter(bad[i]['meshes'][0]['vertices'][0][0],math.inf)
  elif kind=='soft':bad[i]['meshes'][0]['edges'][0]['soft']=not bad[i]['meshes'][0]['edges'][0]['soft']
  else:bad[i]['meshes']=[]
  try:check(bad)
  except AssertionError:pass
  else:raise AssertionError(kind+' mutation admitted')
if len(sys.argv)>1:check(json.loads(Path(sys.argv[1]).read_text()))
print('PASS: 10 exact ordered meshes/fingerprints; compiled flattening negative; 30 mutations rejected')
