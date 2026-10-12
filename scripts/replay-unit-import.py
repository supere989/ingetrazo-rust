#!/usr/bin/env python3
"""Replay the full frozen OBJ/DXF App corpus; strict open winding remains failure."""
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
  folder=repo/'docs/audits/unit-import';m=json.loads((folder/'source-manifest.json').read_text());archive=folder/'source-records.zip'
  assert sha(archive)==m['archive_sha256'];original=(repo/'ingetrazo').resolve()
  pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin==m['source_pin']
  for row in m['original_sources']:assert sha(original/row['path'])==row['sha256'],row['path']
  members={row['path']:row for row in m['members']};assert len(members)==len(m['members'])
  source=out/'source';source.mkdir();native=out/'native';native.mkdir()
  with zipfile.ZipFile(archive)as z:
   assert len(z.namelist())==len(members)and set(z.namelist())==set(members)
   for name,row in members.items():
    assert not Path(name).is_absolute()and '..'not in Path(name).parts
    data=z.read(name);assert len(data)==row['size']and hashlib.sha256(data).hexdigest()==row['sha256'],name
    target=(source/name).resolve();assert target.is_relative_to(source);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
  for row in m['sources']:
   assert sha(source/row['retained_path'])==row['sha256'],row['retained_path']
   assert sha(repo/row['path'])==row['sha256'],row['path']
  # Reused accepted corpora are authenticated independently; no recapture.
  for row in m['reused']:
   parent=repo/row['archive'];assert sha(parent)==row['archive_sha256'],row['archive']
   with zipfile.ZipFile(parent)as z:data=z.read(row['member'])
   assert len(data)==row['size']and hashlib.sha256(data).hexdigest()==row['sha256']
   assert sha(source/'assets'/row['member'])==row['sha256']
  report=json.loads((source/'original-unit-import.json').read_text());assert report['source_pin']==pin and report['callback_errors']==[]
  assert len(report['cases'])==m['cases']==13 and len(report['states'])==m['states']==47
  assert sum(c['saved']is not None for c in report['cases'])==m['saved_readers']==7
  receipt=json.loads((source/m['capture_receipt']).read_text());assert receipt['returncode']==0 and receipt['revision']==m['capture_revision']
  assert binary.is_file()and os.access(binary,os.X_OK),'Prebuilt executable required'
  result.update(source_archive_sha256=m['archive_sha256'],source_pin=pin,capture_receipt=receipt,binary_sha256=sha(binary),revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),scripts={name:sha(repo/'scripts'/name)for name in ['replay-unit-import.py','reference-unit-import-reopens.py','check-unit-import.py']})
  env=dict(os.environ,QT_QPA_PLATFORM='offscreen',INGETRAZO_STATE_DIR=str(out/'state'),XDG_CONFIG_HOME=str(out/'config'),XDG_DATA_HOME=str(out/'data'),XDG_CACHE_HOME=str(out/'cache'))
  for name,argv in [('native',[binary,source,native]),('reader',[sys.executable,repo/'scripts/reference-unit-import-reopens.py',source,native]),('checker',[sys.executable,repo/'scripts/check-unit-import.py',source,native,out/'comparison.json'])]:
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
