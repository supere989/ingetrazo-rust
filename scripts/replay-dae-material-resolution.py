#!/usr/bin/env python3
"""Replay authenticated original COLLADA material inputs with a prebuilt consumer."""
import argparse, hashlib, json, os, subprocess, sys, zipfile
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('binary',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
repo=Path(__file__).resolve().parents[1];folder=repo/'docs/audits/dae-material-resolution'
receipt=json.loads((folder/'receipt.json').read_text())
for name in ['source-records.zip','source-manifest.json']:
    data=(folder/name).read_bytes();assert len(data)==receipt[name]['bytes'] and hashlib.sha256(data).hexdigest()==receipt[name]['sha256'],name
manifest=json.loads((folder/'source-manifest.json').read_text());source=a.output/'source'
assert not source.exists() and not (a.output/'native').exists(), 'retain each replay in a new output directory'
source.mkdir(parents=True)
with zipfile.ZipFile(folder/'source-records.zip') as z:
    assert {i.filename for i in z.infolist() if not i.is_dir()}==set(manifest)
    for info in z.infolist():
        target=source/info.filename;target.resolve().relative_to(source.resolve())
        if info.is_dir():target.mkdir(parents=True,exist_ok=True);continue
        data=z.read(info);expected=manifest[info.filename]
        assert len(data)==expected['bytes'] and hashlib.sha256(data).hexdigest()==expected['sha256'],info.filename
        target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
pin=json.loads((source/'source.json').read_text())['source_commit']
assert subprocess.check_output(['git','-C',str(repo/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
subprocess.run(['git','-C',str(repo/'ingetrazo'),'diff','--quiet','HEAD'],check=True)
env=dict(os.environ,QT_QPA_PLATFORM='offscreen',PYTHONHASHSEED='0')
subprocess.run([sys.executable,str(repo/'scripts/check-dae-material-resolution.py'),'replay',str(source.resolve()),'--output',str((a.output/'native').resolve()),'--binary',str(a.binary.resolve())],cwd=repo,env=env,check=True)
