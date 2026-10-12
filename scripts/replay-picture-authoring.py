#!/usr/bin/env python3
"""Authenticate retained source and replay a prebuilt native picture authoring consumer."""
import argparse, hashlib, json, os, subprocess, sys, time, zipfile
from pathlib import Path

def digest(path):
    with path.open('rb')as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('binary',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args()
    repo=Path(__file__).resolve().parents[1];folder=repo/'docs/audits/picture-authoring';out=args.output.resolve();binary=args.binary.resolve()
    assert not out.exists(),'Replay requires a fresh output directory'
    out.mkdir(parents=True);source=out/'source';native=out/'native';source.mkdir();native.mkdir()
    manifest=json.loads((folder/'source-manifest.json').read_text());archive=folder/'source-records.zip'
    assert digest(archive)==manifest['archive_sha256']
    pin=subprocess.check_output(['git','-C',str(repo/'ingetrazo'),'rev-parse','HEAD'],text=True).strip();assert pin==manifest['original_commit']
    with zipfile.ZipFile(archive)as z:
        assert len(z.namelist())==len(manifest['members'])and set(z.namelist())==set(manifest['members'])
        for name,record in manifest['members'].items():
            data=z.read(name);assert len(data)==record['size']and hashlib.sha256(data).hexdigest()==record['sha256'],name
            path=source/name;path.resolve().relative_to(source);path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
    env=dict(os.environ,QT_QPA_PLATFORM='offscreen',INGETRAZO_STATE_DIR=str(out/'state'),XDG_CONFIG_HOME=str(out/'config'),XDG_DATA_HOME=str(out/'data'),XDG_CACHE_HOME=str(out/'cache'))
    result={'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'binary_sha256':digest(binary),'source_pin':pin,'source_archive_sha256':manifest['archive_sha256'],'fresh_source_captures':0,'commands':[],'result':'incomplete'}
    try:
        for label,argv in [('native',[binary,source,native]),('reader',[sys.executable,repo/'scripts/reference-picture-authoring-reopens.py',source,native]),('checker',[sys.executable,repo/'scripts/check-picture-authoring.py',source,native,out/'comparison.json'])]:
            command=list(map(str,argv));started=time.monotonic()
            with (out/(label+'.log')).open('w')as stdout,(out/(label+'.stderr')).open('w')as stderr:
                run=subprocess.run(command,cwd=source,env=env,stdout=stdout,stderr=stderr,timeout=180)
            result['commands'].append({'argv':command,'cwd':str(source),'seconds':time.monotonic()-started,'returncode':run.returncode});run.check_returncode()
        result['result']='pass'
    except Exception as error:result['result']='fail';result['error']=str(error)
    finally:(out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result));return 0 if result['result']=='pass'else 1
if __name__=='__main__':raise SystemExit(main())
