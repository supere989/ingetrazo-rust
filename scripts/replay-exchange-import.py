#!/usr/bin/env python3
"""Replay the full frozen DAE/GLB App corpus; strict open winding remains failure."""
import argparse,hashlib,json,os,subprocess,sys,time,zipfile
from pathlib import Path

def sha(path):
 with path.open('rb')as f:return hashlib.file_digest(f,'sha256').hexdigest()
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('binary',type=Path);ap.add_argument('output',type=Path);args=ap.parse_args()
 repo=Path(__file__).resolve().parents[1];out=args.output.resolve();binary=args.binary.resolve()
 assert not out.exists(),'Fresh output required';out.mkdir(parents=True)
 result={'passed':False,'fresh_source_captures':0,'strict_winding_exemptions':0,'steps':[]}
 try:
  folder=repo/'docs/audits/exchange-import';m=json.loads((folder/'source-manifest.json').read_text());archive=folder/'source-records.zip'
  assert sha(archive)==m['archive_sha256'];original=(repo/'ingetrazo').resolve()
  pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin==m['original_commit']
  for row in [x for x in m['verified_source_support'] if x.get('commit')==pin]:assert sha(original/row['path'])==row['sha256'],row['path']
  members=m['members']
  source=out/'source';source.mkdir();native=out/'native';native.mkdir()
  with zipfile.ZipFile(archive)as z:
   assert len(z.namelist())==len(members)and set(z.namelist())==set(members)
   for name,row in members.items():
    assert not Path(name).is_absolute()and '..'not in Path(name).parts
    data=z.read(name);assert len(data)==row['size']and hashlib.sha256(data).hexdigest()==row['sha256'],name
    target=(source/name).resolve();assert target.is_relative_to(source);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
  for row in m['sources']:
   assert sha(repo/row['path'])==row['sha256'],row['path']
  for row in m['capture_sources']:
   assert sha(source/row['retained_member'])==row['sha256'],row['retained_member']
  for alias,row in m['identical_file_aliases'].items():
   retained=source/row['retained_member']
   assert retained.stat().st_size==row['size'] and sha(retained)==row['sha256'],alias
  report=json.loads((source/'original-exchange.json').read_text());assert report['source_pin']==pin
  assert len(report['cases'])==m['accepted_counts']['cases']==13 and len(report['states'])==m['accepted_counts']['states']==61
  assert sum(c['saved'] is not None for c in report['cases'])==m['accepted_counts']['saved_original_igz']==11
  assert binary.is_file()and os.access(binary,os.X_OK),'Prebuilt executable required'
  result.update(source_archive_sha256=m['archive_sha256'],source_pin=pin,binary_sha256=sha(binary),revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),scripts={name:sha(repo/'scripts'/name)for name in ['replay-exchange-import.py','reference-exchange-import-reopens.py','check-exchange-import.py']})
  env=dict(os.environ,QT_QPA_PLATFORM='offscreen',INGETRAZO_STATE_DIR=str(out/'state'),XDG_CONFIG_HOME=str(out/'config'),XDG_DATA_HOME=str(out/'data'),XDG_CACHE_HOME=str(out/'cache'))
  for name,argv in [('native',[binary,source,native]),('reader',[sys.executable,repo/'scripts/reference-exchange-import-reopens.py',source,native]),('checker',[sys.executable,repo/'scripts/check-exchange-import.py',source,native,out/'comparison.json'])]:
   argv=list(map(str,argv));started=time.monotonic()
   with (out/(name+'.log')).open('w')as log:
    try:r=subprocess.run(argv,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=180)
    except subprocess.TimeoutExpired:
     result['steps'].append({'name':name,'argv':argv,'seconds':time.monotonic()-started,'timeout':180});raise
   result['steps'].append({'name':name,'argv':argv,'seconds':time.monotonic()-started,'returncode':r.returncode});r.check_returncode()
  result['passed']=True
 except Exception as e:result['error']=repr(e)
 finally:(out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result));return 0 if result['passed']else 1
if __name__=='__main__':raise SystemExit(main())
