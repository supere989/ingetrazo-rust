#!/usr/bin/env python3
"""Strict original/native referenced-asset boundary; no ZIP cache-purge claim."""
import hashlib, json, sys, zipfile
from pathlib import Path

def check(original, native):
    src=json.loads((original/'original.json').read_text())
    dst=json.loads((native/'native.json').read_text())
    assert src['provenance']['original_python_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
    assert dst['preview_cancel_verified'] and dst['system_clipboard_disabled']
    assert set(src['cases'])==set(dst['cases'])=={'direct','nested'}
    for name, rows in src['cases'].items():
        actual=dst['cases'][name]
        assert len(rows)==len(actual)==3
        for expected, observed in zip(rows,actual):
            for key in ('phase','faces','textured_faces','uvw','vendor'):
                assert expected[key]==observed[key], (name,key,expected,observed)
            assert len(observed['asset_refs'])==expected['textured_faces']
            with zipfile.ZipFile(native/f"{name}-{observed['phase']}.igz") as archive:
                hashes=[hashlib.sha256(archive.read(ref)).hexdigest() for ref in observed['asset_refs']]
            assert hashes==expected['texture_hashes'], (name,observed['phase'],hashes)
    return 6
if __name__=='__main__':
    original,native,baseline=map(Path,sys.argv[1:4])
    count=check(original,native)
    try: check(original,baseline)
    except (KeyError,zipfile.BadZipFile) as exc:
        # No embedded assets makes the writer emit plain JSON despite refs.
        assert isinstance(exc,zipfile.BadZipFile) or 'textures/' in str(exc)
    else: raise AssertionError('Missing-image baseline was accepted')
    print(json.dumps({'exact_states':count,'missing_archive_asset_negative':True,'scope':'referenced PNG bytes, geometry/UVW, app preview cancellation; unreferenced cache packaging not compared'}))
