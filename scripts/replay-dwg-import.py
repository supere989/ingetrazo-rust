#!/usr/bin/env python3
"""Authenticate frozen DWG inputs and run a prebuilt native import consumer."""
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
        folder=repo/'docs/audits/dwg-import';m=json.loads((folder/'source-manifest.json').read_text())
        archive=folder/'source-records.zip';assert sha(archive)==m['archive_sha256']
        original=(repo/'ingetrazo').resolve()
        pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()
        assert pin==m['original_commit']
        for key,base in [('sources',repo),('original_sources',original)]:
            for v in m[key]:assert sha(base/v['path'])==v['sha256'],v['path']
        for v in m['capture_sources']:
            assert sha(repo/v['retained_path'])==v['sha256'],v['retained_path']
        assert sha(repo/m['converter']['path'])==m['converter']['sha256']
        source=out/'source';source.mkdir();native=out/'native';native.mkdir()
        with zipfile.ZipFile(archive) as z:
            assert len(z.namelist())==len(m['members']) and set(z.namelist())==set(m['members'])
            for name,v in m['members'].items():
                b=z.read(name);assert len(b)==v['size'] and hashlib.sha256(b).hexdigest()==v['sha256'],name
                p=(source/name).resolve();assert p.is_relative_to(source);p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
        result.update(source_archive_sha256=m['archive_sha256'],source_pin=pin,binary_sha256=sha(binary),
                      revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip())
        env=dict(os.environ,QT_QPA_PLATFORM='offscreen',INGETRAZO_STATE_DIR=str(out/'state'),XDG_CONFIG_HOME=str(out/'config'),XDG_DATA_HOME=str(out/'data'),XDG_CACHE_HOME=str(out/'cache'))
        for label,command in [('native',[binary,source,native]),('reader',[sys.executable,repo/'scripts/reference-dwg-import-reopens.py',source,native]),('checker',[sys.executable,repo/'scripts/check-dwg-import.py',source,native,out/'comparison.json'])]:
            argv=list(map(str,command));start=time.monotonic()
            with (out/(label+'.log')).open('w') as f:
                r=subprocess.run(argv,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=180)
            result['steps'].append({'argv':argv,'seconds':time.monotonic()-start,'returncode':r.returncode})
            r.check_returncode()
        result['ok']=True
    except Exception as e:result['error']=repr(e)
    finally:(out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result));return 0 if result['ok'] else 1
if __name__=='__main__':raise SystemExit(main())
