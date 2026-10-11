#!/usr/bin/env python3
"""Strict retained source24 frames plus actual Metal controller/cache ownership."""
import argparse,copy,hashlib,json,zipfile
from pathlib import Path
def validate(source,native):
 assert native['samples']==4 and native['ledger']==0 and native['device']
 assert [r['name'] for r in native['cases']]==[r['name'] for r in source['cases']]
 assert len(native['cases'])==6
 for row in native['cases']:
  assert row['model_unchanged'] and row['warm_uploads_zero'] and row['pressure_old_pixels_charge_retry']
  assert [p['phase'] for p in row['phases']]==['base','preview','offset','cancel']
def equal(a,b):assert len(a)==len(b)==800*600*4 and a==b,'Strict whole RGBA differs'
def main():
 p=argparse.ArgumentParser();p.add_argument('audit');p.add_argument('output');p.add_argument('--unpack',action='store_true');a=p.parse_args();root=Path(a.audit);out=Path(a.output)
 manifest=json.loads((root/'manifest.json').read_text());archive=root/'records.zip'
 assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
 with zipfile.ZipFile(archive) as z:
  files={r['path']:r['sha256'] for r in manifest.get('entries',manifest.get('records'))};assert set(z.namelist())==set(files)
  for n,h in files.items():assert hashlib.sha256(z.read(n)).hexdigest()==h,n
  prefix='complete-config/' if 'complete-config/original.json' in files else '';source=json.loads(z.read(prefix+'original.json'));assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
  assert len(source['cases'])==6 and len({r['name'] for r in source['cases']})==6
  if a.unpack:
   out.mkdir(parents=True,exist_ok=True)
   for n in z.namelist():
    if prefix and not n.startswith(prefix):continue
    dest=out/n.removeprefix(prefix);assert dest.resolve().is_relative_to(out.resolve());dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(z.read(n))
   return
  from PIL import Image
  import io
  native=json.loads((out/'results.json').read_text());validate(source,native)
  mutations=[lambda r:r.update(samples=1),lambda r:r.update(ledger=1),lambda r:r['cases'].pop(),lambda r:r['cases'][0].update(warm_uploads_zero=False)]
  for mutate in mutations:
   bad=copy.deepcopy(native);mutate(bad)
   try:validate(source,bad)
   except (AssertionError,KeyError):pass
   else:raise AssertionError('Malformed native acceptance admitted')
  errors=[]
  for case in source['cases']:
   assert case['gl']['vendor']=='NVIDIA Corporation' and 'RTX 5090' in case['gl']['renderer']
   for row in case['frames']:
    assert row['samples']==4
    im=Image.open(io.BytesIO(z.read(prefix+row['file']))).convert('RGBA');assert im.size==(800,600);expected=im.tobytes();actual=(out/(case['name']+'-'+row['phase']+'.rgba')).read_bytes()
    try:equal(expected,actual)
    except AssertionError:
     assert len(actual)==len(expected)
     errors.append({'case':case['name'],'phase':row['phase'],'different_pixels':sum(expected[i:i+4]!=actual[i:i+4] for i in range(0,len(actual),4)),'maximum_channel_error':max(abs(x-y) for x,y in zip(expected,actual))})
  # Exercise the exact comparator with a literal changed native pixel.
  altered=bytearray(expected);altered[0]^=1
  try:equal(expected,altered)
  except AssertionError:pass
  else:raise AssertionError('Mutated pixel accepted')
  print(json.dumps({'functional_passed':True,'strict_pixels_passed':not errors,'frames':24,'negative_controls':5,'differences':errors},indent=2))
  if errors:raise SystemExit(1)
if __name__=='__main__':main()
