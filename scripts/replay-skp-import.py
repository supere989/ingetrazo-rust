#!/usr/bin/env python3
"""Authenticate frozen SKP inputs and run a prebuilt native import consumer."""
import argparse, hashlib, json, os, subprocess, sys, time, zipfile
from pathlib import Path

def sha(path):
    with path.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('binary',type=Path);ap.add_argument('output',type=Path);args=ap.parse_args()
    repo=Path(__file__).resolve().parents[1];out=args.output.resolve();binary=args.binary.resolve()
    assert not out.exists(),'Fresh output directory required';out.mkdir(parents=True)
    result={'ok':False,'fresh_source_captures':0,'steps':[]}
    try:
        folder=repo/'docs/audits/skp-import';m=json.loads((folder/'source-manifest.json').read_text())
        archive=folder/'source-records.zip';assert sha(archive)==m['archive_sha256']
        original=(repo/'ingetrazo').resolve()
        pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()
        assert pin==m['original_commit']
        contract=json.loads((folder/'fixture-contract.json').read_text())
        assert contract['source_pin']==pin and contract['parser_pin']==m['parser_commit']
        for v in m['sources']:assert sha(repo/v['path'])==v['sha256'],v['path']
        for v in contract['original_sources']+[m['failure_source_binding']]:
            assert sha(original/v['path'])==v['sha256'],v['path']
        source=out/'source';source.mkdir();native=out/'native';native.mkdir()
        with zipfile.ZipFile(archive) as z:
            assert len(z.namelist())==len(m['members']) and set(z.namelist())==set(m['members'])
            for name,v in m['members'].items():
                b=z.read(name);assert len(b)==v['size'] and hashlib.sha256(b).hexdigest()==v['sha256'],name
                assert not Path(name).is_absolute() and '..' not in Path(name).parts
                p=(source/name).resolve();assert p.is_relative_to(source);p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
        for name,v in m['identical_file_aliases'].items():
            assert v['retained_member'] in m['members'],name
            retained=source/v['retained_member']
            assert retained.stat().st_size==v['size'] and sha(retained)==v['sha256'],name
        for v in m['capture_sources']:
            name=v['retained_member']
            if name not in m['members']:
                alias=m['identical_file_aliases'][name]
                assert alias['sha256']==v['sha256'],name
                name=alias['retained_member']
            assert sha(source/name)==v['sha256'],name
        identity=json.loads((source/'parser-identity.json').read_text())
        assert identity['pin']==m['parser_commit'] and identity['sources']==contract['parser_sources']
        frozen=json.loads((source/'original-skp.json').read_text())
        assert frozen['source_pin']==pin
        assert len(frozen['cases'])==contract['expected_cases'] and len(frozen['states'])==contract['expected_states']
        assert not frozen['callback_errors']
        assert binary.is_file() and os.access(binary,os.X_OK), 'Prebuilt executable required'
        result.update(source_archive_sha256=m['archive_sha256'],source_pin=pin,binary_sha256=sha(binary),
                      replay_scripts={name:sha(repo/'scripts'/name) for name in ['replay-skp-import.py','reference-skp-import-reopens.py','check-skp-import.py']},
                      revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip())
        env=dict(os.environ,QT_QPA_PLATFORM='offscreen',INGETRAZO_STATE_DIR=str(out/'state'),XDG_CONFIG_HOME=str(out/'config'),XDG_DATA_HOME=str(out/'data'),XDG_CACHE_HOME=str(out/'cache'))
        for label,command in [('native',[binary,source,native]),('reader',[sys.executable,repo/'scripts/reference-skp-import-reopens.py',source,native]),('checker',[sys.executable,repo/'scripts/check-skp-import.py',source,native,out/'comparison.json'])]:
            argv=list(map(str,command));start=time.monotonic()
            with (out/(label+'.log')).open('w') as f:
                try:
                    r=subprocess.run(argv,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=180)
                except subprocess.TimeoutExpired:
                    result['steps'].append({'argv':argv,'seconds':time.monotonic()-start,'timeout':180})
                    raise
            result['steps'].append({'argv':argv,'seconds':time.monotonic()-start,'returncode':r.returncode})
            r.check_returncode()
        result['ok']=True
    except Exception as e:result['error']=repr(e)
    finally:(out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result));return 0 if result['ok'] else 1
if __name__=='__main__':raise SystemExit(main())
