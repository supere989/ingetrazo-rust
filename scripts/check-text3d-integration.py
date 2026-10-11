#!/usr/bin/env python3
"""Read-only authentication and fresh integrated font/Scale comparison."""
import hashlib, importlib.util, json, zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
audit=root/'docs/audits/text3d-integration'
m=json.loads((audit/'manifest.json').read_text())
raw=(audit/'records.zip').read_bytes()
assert len(raw)==m['archive_bytes'] and hashlib.sha256(raw).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(audit/'records.zip') as z:
    assert set(z.namelist())==set(m['records'])
    for name,r in m['records'].items():
        data=z.read(name);assert len(data)==r['bytes'] and hashlib.sha256(data).hexdigest()==r['sha256'],name
    summary=json.loads(z.read('workspace-summary.json'))
    assert summary==m['workspace'] and summary['passed']==933 and summary['failed']==0 and summary['ignored']==2 and summary['cargo_exit']==0
    assert z.read('workflow.log').strip()==b'3D Text application workflow passed'
    reopen=json.loads(z.read('original-reopen.json'))
    assert reopen['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
    assert [r['name']for r in reopen['rows']]==['placed','edited','context-flat','fingerprint-boundary']
    assert all(r['pristine'] and len(r['hash'])==16 for r in reopen['rows'])
    font=json.loads(z.read('font-native.json'));scale=json.loads(z.read('scale-native.json'))
with zipfile.ZipFile(root/'docs/audits/text3d-font/records.zip') as z:
    source=json.loads(z.read('original.json'))
assert len(font)==10 and [x['name']for x in font]==[x['name']for x in source['cases']]
for a,b in zip(source['cases'],font):
    assert a['meshes']==b['meshes'] and a['fingerprint']==b['state']['text3d']['hash'],a['name']
spec=importlib.util.spec_from_file_location('scale_check',root/'scripts/check-scale-rapid-context.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
assert module.compare(json.loads((root/'docs/audits/scale-rapid-context/source.json').read_text()),scale)==52
print('PASS: 933 tests; 10 exact font meshes/fingerprints; app workflow; 4 original reopens; 52 exact Scale states')
