#!/usr/bin/env python3
"""Authenticate frozen inputs and run the actual Metal Host/TCP probe only."""
import hashlib,json,pathlib,subprocess,sys,time,zipfile,tempfile
repo=pathlib.Path(__file__).resolve().parents[1]
binary=pathlib.Path(sys.argv[1]).resolve(); out=pathlib.Path(sys.argv[2]).resolve()
if out.exists(): raise SystemExit('Output must be fresh')
out.mkdir(parents=True)
manifest_path=repo/'docs/audits/ai-bridge-capture/source-manifest.json'
archive=manifest_path.with_name('source-records.zip')
m=json.loads(manifest_path.read_text()); sha=lambda b:hashlib.sha256(b).hexdigest()
receipt={'scope':'Actual Metal host lifecycle probe; no source pixel parity claim','revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'manifest_sha256':sha(manifest_path.read_bytes()),'steps':[]}
try:
    assert m['original_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
    assert sha(archive.read_bytes())==m['archive_sha256']
    source=pathlib.Path(tempfile.mkdtemp(prefix='ingetrazo-metal-capture-source-'))
    receipt['source_directory']=str(source)
    with zipfile.ZipFile(archive) as z:
        assert len(z.namelist())==len(set(z.namelist()))
        assert set(z.namelist())==set(m['members'])
        for name,spec in m['members'].items():
            rel=pathlib.PurePosixPath(name)
            assert not rel.is_absolute() and '..' not in rel.parts
            b=z.read(name); assert len(b)==spec['size'] and sha(b)==spec['sha256']
            dest=source/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(b)
        for binding in m['capture_sources']:
            assert sha((source/binding['retained_path']).read_bytes())==binding['sha256']
    receipt['archive_sha256']=m['archive_sha256'];receipt['authenticated_members']=len(m['members'])
    receipt['binary_sha256']=sha(binary.read_bytes())
    started=time.monotonic()
    with (out/'native.log').open('w') as log:
        try:
            r=subprocess.run([str(binary),str(source),str(out/'native')],stdout=log,stderr=subprocess.STDOUT,timeout=75)
            code=r.returncode
        except subprocess.TimeoutExpired:
            code=124
    receipt['steps'].append({'name':'actual-host-tcp','returncode':code,'seconds':time.monotonic()-started,'timeout_seconds':75})
    if code: raise RuntimeError(f'Native probe failed: {code}')
    report=json.loads((out/'native/report.json').read_text())
    receipt['native_report_sha256']=sha((out/'native/report.json').read_bytes())
    receipt['ok']=True
except Exception as error:
    receipt['ok']=False;receipt['error']=str(error)
finally:
    (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
raise SystemExit(0 if receipt['ok'] else 1)
