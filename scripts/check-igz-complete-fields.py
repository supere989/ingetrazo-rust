#!/usr/bin/env python3
"""Read-only verification of source writer/reader format acceptance."""
import hashlib, json, zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'docs/audits/igz-complete-fields'
manifest=json.loads((root/'manifest.json').read_text())
archive=root/'records.zip'
assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
with zipfile.ZipFile(archive) as z:
    assert set(z.namelist())==set(manifest['records'])
    for name,digest in manifest['records'].items():assert hashlib.sha256(z.read(name)).hexdigest()==digest,name
    expected=json.loads(z.read('expected.json'))
    observed=json.loads(z.read('native/observed.json'))
    assert expected['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
    assert len(expected['source_keys'])==28
    assert set(expected['cases'])=={'no-op','loose-edit','rename-parent','move-child','remove-copy','paint'}
    assert expected['cases']==observed
    assert all(not x for x in json.loads(z.read('native/differences.json')).values())
    for case,scene in observed.items():assert set(expected['source_keys'])<=set(scene),case
    controls=json.loads(z.read('controls.json'))
    assert set(controls)=={'custom-scales','saved-view-uid','group-extension','photo-anchor'}
    for name,control in controls.items():
        assert control['rejected'] and control['observed']!=observed['no-op'],name
print('PASS: 28 source top-level fields, six exact edit/reopen cases, four archive corruptions rejected')
