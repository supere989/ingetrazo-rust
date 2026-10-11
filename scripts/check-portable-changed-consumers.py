#!/usr/bin/env python3
"""Strict source images plus recorded-failure authentication for changed passes."""
import hashlib,io,json,sys,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[1];base=root/'docs/audits/portable-changed-consumers'
m=json.loads((base/'manifest.json').read_text());assert hashlib.sha256((base/'records.zip').read_bytes()).hexdigest()==m['archive_sha256']
z=zipfile.ZipFile(base/'records.zip');assert set(z.namelist())==set(m['records'])
for name,v in m['records'].items():
 raw=z.read(name);assert len(raw)==v['bytes']and hashlib.sha256(raw).hexdigest()==v['sha256']
def source_archive(directory):
 d=root/'docs/audits'/directory;manifest=json.loads((d/'manifest.json').read_text());raw=(d/'captures.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==manifest['archive_sha256'];return zipfile.ZipFile(io.BytesIO(raw))
solar=source_archive('cutout-discard-portable-depth');ghost=source_archive('hidden-ghost-workflow')
def source(kind,name):
 raw=solar.read('source/retained/'+name)if kind=='solar'else ghost.read('original/'+name)if kind=='ghost'else(root/'docs/audits/native-image-planes/original-nvidia'/name).read_bytes()
 assert hashlib.sha256(raw).hexdigest()==m['input_sha256'][kind+'/'+name];return raw
sources={kind:json.loads(source(kind,'original.json'if kind=='solar'else'original-alpha-results.json'))for kind in ['solar','image','ghost']}
def pixels(raw):return np.array(Image.open(io.BytesIO(raw)).convert('RGBA'))
def compare(a,b):
 assert a.shape==b.shape
 return int(np.any(a!=b,axis=2).sum()),int(np.abs(a.astype(np.int16)-b.astype(np.int16)).max())
executions=json.loads(z.read('executions.json'));assert len(executions)==10 and all(r['returncode']==0 for r in executions)
rows=[]
for mode in ['native','portable']:
 for name in ['solid','section','cutout']:
  c=next(c for c in sources['solar']['cases']if c['name']==name)
  assert c['receiver_samples']==4
  result=json.loads(z.read(f'{mode}/solar/{name}/result.json'));assert result['ledger_zero']and result['refusal']and result['samples']==4
  assert result['billboard_cold']['native_gl_depth_control']==(mode=='native')
  for phase in ['on','off','restored','depth']:
   ref=c['depth_image']if phase=='depth'else next(v['image']for v in c['images']if v['phase']==phase)
   count,maximum=compare(pixels(source('solar',ref)),pixels(z.read(f'{mode}/solar/{name}/{phase}.png')))
   rows.append(dict(kind='solar',mode=mode,case=name,phase=phase,pixels=count,max=maximum))
 for kind,names in [('image',{'image-quadrants','image-overlap','image-opacity-below','image-opaque-behind','image-coplanar-trace'}),('ghost',{'face-on','edge-on','textured-object-on'})]:
  result=json.loads(z.read(f'{mode}/{kind}/native-alpha-results.json'));assert len(result['cases'])==len(names)and{r['case']for r in result['cases']}==names
  for c in sorted(result['cases'],key=lambda c:c['case']):
   original=next(v for v in sources[kind]['cases']if v['case']==c['case']);assert c['samples']==original['samples']
   count,maximum=compare(pixels(source(kind,c['case']+'.png')),pixels(z.read(f'{mode}/{kind}/'+c['case']+'.png')))
   rows.append(dict(kind=kind,mode=mode,case=c['case'],phase='frame',pixels=count,max=maximum))
recorded=json.loads(z.read('comparison.json'));assert rows==recorded and len(rows)==40
for r in rows:
 expected=(38486,255)if r['kind']=='solar'and r['mode']=='portable'and r['case']=='section'and r['phase']=='depth'else(1212,135)if r['kind']=='image'else(0,0)
 assert(r['pixels'],r['max'])==expected,r
for r in rows:
 if r['mode']!='portable':continue
 suffix=f"solar/{r['case']}/{r['phase']}.png"if r['kind']=='solar'else f"{r['kind']}/{r['case']}.png"
 pair=compare(pixels(z.read('native/'+suffix)),pixels(z.read('portable/'+suffix)))
 assert pair==((38486,255)if suffix=='solar/section/depth.png'else(0,0))
 # Literal one-byte/blank controls exercise the same full-image comparator.
 image=pixels(z.read('portable/'+suffix));changed=image.copy();changed[0,0,0]^=1;assert compare(image,changed)[0]==1
 assert compare(image,np.zeros_like(image))[0]>0
print('Authenticated 40 full-image records:29 exact;80 source samples exact;19/20 native/portable image pairs exact. Retained section-map and reference-image strict failures.')
if '--verify-recorded'not in sys.argv:raise SystemExit(1)
