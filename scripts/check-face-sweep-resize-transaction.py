#!/usr/bin/env python3
"""Read-only strict resize/refusal archive authentication."""
import hashlib,json,sys,zipfile
from pathlib import Path
root=Path(sys.argv[1]);m=json.loads((root/'manifest.json').read_text())
assert hashlib.sha256((root/'records.zip').read_bytes()).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(root/'records.zip')as z:
 assert set(z.namelist())==set(m['records'])
 for name,v in m['records'].items():
  raw=z.read(name);assert len(raw)==v['bytes']and hashlib.sha256(raw).hexdigest()==v['sha256']
 report=json.loads(z.read('final/result.json'));assert report['passed']and report['source_sha']==m['compiled_source']
 rows=report['rows'];assert len(rows)==17
 states={r['state']['nonce']:r['state']for r in rows}
 for before,refused,retry in [(3,5,7),(7,9,11),(11,13,15)]:
  assert z.read(f'final/control/target-{before}.rgba')==z.read(f'final/control/target-{refused}.rgba')
  a,b,c=[states[n]for n in [before,refused,retry]]
  assert a['world']==b['world']and states[refused-1]['submitted_serial']==b['submitted_serial']
  assert b['prepare_error']and not c['prepare_error']
  assert c['world']['extent']!=a['world']['extent']and c['completed_present_serial']>a['submitted_serial']
  if refused!=13:assert b['old_label_owner_alive']
  assert len(z.read(f'final/control/target-{retry}.rgba'))==c['world']['extent'][0]*c['world']['extent'][1]*4
 assert b'committed_bytes=0'in z.read('final/runtime.log')
 for negative in ['first-wsi-charge-negative','warm-cad-control-negative']:
  assert json.loads(z.read(negative+'/transport.json'))['returncode']==1
print('PASS three actual OS-resize refusals preserve retained World RGBA/owners; each retry accepts new extent')
