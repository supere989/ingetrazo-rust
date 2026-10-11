#!/usr/bin/env python3
"""Literal pinned Scale controller inventory; no numeric tolerance."""
import copy,hashlib,json,sys,zipfile
from pathlib import Path
audit=Path(sys.argv[1])
manifest=json.loads((audit/'manifest.json').read_text())
archive=audit/'records.zip'
assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
with zipfile.ZipFile(archive) as z:
    for name,digest in manifest['files'].items():
        assert hashlib.sha256(z.read(name)).hexdigest()==digest
    source=json.loads(z.read('source.json'))
    native=json.loads(Path(sys.argv[2]).read_text()) if len(sys.argv)>2 else json.loads(z.read('native.json'))
def validate(actual):
    assert actual['cases']==source['cases'],'Scale source/controller inventory differs'
validate(native)
bad=copy.deepcopy(native);bad['cases'][0]['controls'][0]['anchor'][0]+=.01
mutations=[bad]
bad=copy.deepcopy(native);bad['cases'][0]['grip_count']-=1;mutations.append(bad)
bad=copy.deepcopy(native);bad['cases'][3]['controls'][0]['ray_factors'][0]+=.000001;mutations.append(bad)
bad=copy.deepcopy(native);bad['cases'][1]['controls'][0]['typed'][0]['factors'][2]=2.;mutations.append(bad)
for bad in mutations:
    try:validate(bad)
    except AssertionError:pass
    else:raise AssertionError('Corrupted Scale inventory accepted')
print(json.dumps({'boxes':len(source['cases']),'exact_grip_modifier_controls':sum(len(c['controls'])for c in source['cases']),'negative_controls':len(mutations),'scope':'Source controller math only; full native tool/input/rendering integration pending'}))
