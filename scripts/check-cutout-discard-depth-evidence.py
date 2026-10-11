#!/usr/bin/env python3
"""Strict compact source/native replay; retain portable packed-map failure."""
import hashlib,io,json,pathlib,zipfile
from PIL import Image
import numpy as np
base=pathlib.Path(__file__).resolve().parents[1]/'docs/audits/cutout-discard-portable-depth'
m=json.loads((base/'manifest.json').read_text());z=zipfile.ZipFile(base/'captures.zip')
assert hashlib.sha256((base/'captures.zip').read_bytes()).hexdigest()==m['archive_sha256']
assert set(z.namelist())==set(m['files'])
for name,digest in m['files'].items():assert hashlib.sha256(z.read(name)).hexdigest()==digest,name
read=lambda name:json.loads(z.read(name))
image=lambda name:np.array(Image.open(io.BytesIO(z.read(name))).convert('RGBA'))
def compare(a,b):
 assert a.shape==b.shape
 return int(np.any(a!=b,axis=2).sum()),int(np.abs(a.astype(int)-b.astype(int)).max())
def inventory(cases,names):assert {c['name']for c in cases}==names and len(cases)==len(names)
retained=read('source/retained/original.json');heldout=read('source/heldout/original.json')
for source in [retained,heldout]:assert source['source_pin']==m['source_pin'] and source['qt']=='6.12.0'
portable_names={'billboard-mesh-placement-order','billboard-bitmap-empty-parent','billboard-bitmap-nested-reflection','solid','section','cutout','billboard-bitmap-hardcutout','billboard-bitmap-perspective'}
heldout_names={'cutout-perspective','cutout-quarter-alpha','cutout-threshold-alpha'}
inventory(retained['cases'],portable_names);inventory(heldout['cases'],heldout_names)
rows=[]
for mode,source,prefix,only in [('native',retained,'source/retained',{'cutout'}),('heldout',heldout,'source/heldout',None),('portable',retained,'source/retained',None),('legacy',retained,'source/retained',{'cutout'})]:
 native={'native':'default','heldout':'heldout','portable':'portable','legacy':'legacy'}[mode]
 transport=read('native/'+native+'/transport.json');assert transport['source_commit']==m['renderer_source']
 expected={c['name']for c in source['cases']if only is None or c['name']in only};assert {r['case']for r in transport['cases']}==expected
 for c in source['cases']:
  if only is not None and c['name']not in only:continue
  assert 'NVIDIA' in c['gl']['vendor'] and c['receiver_samples']==4
  assert hashlib.sha256(z.read(prefix+'/'+c['igz'])).hexdigest()==c['igz_sha256']
  result=read('native/'+native+'/'+c['name']+'/result.json');assert result['source_commit']==m['renderer_source'] and result['ledger_zero'] and result['refusal'] and result['samples']==4
  stats=result['billboard_cold'];assert stats['source_terminate_discard']==(mode!='legacy') and stats['native_gl_depth_control']==(mode!='portable')
  for item in c['images']+[{'phase':'depth','image':c['depth_image'],'sha256':c['depth_sha256']}]:
   path=prefix+'/'+item['image'];assert hashlib.sha256(z.read(path)).hexdigest()==item['sha256']
   a=image(path);b=image('native/'+native+'/'+c['name']+'/'+item['phase']+'.png');assert a.shape==((2048,2048,4)if item['phase']=='depth'else(600,800,4))
   count,maximum=compare(a,b);rows.append({'mode':mode,'case':c['name'],'phase':item['phase'],'pixels':count,'max':maximum})
assert rows==json.loads((base/'comparison.json').read_text()) and len(rows)==52
for row in rows:
 if row['mode'] in {'native','heldout'}:assert row['pixels']==0,row
 if row['mode']=='legacy':assert row['pixels']==(286 if row['phase']in {'on','restored'}else 0),row
 if row['mode']=='portable' and not(row['case']=='section'and row['phase']=='depth'):assert row['pixels']==0,row
for normal in ['normal','normal-portable','solar','solar-portable']:
 app=read('application/'+normal+'/app/result.json');assert app['passed'] and len(app['rows'])==8
 assert app['source_sha']==(m['solar_app_probe_source']if normal.startswith('solar')else m['renderer_source'])
 for i,row in enumerate(app['rows'],1):
  state=row['state'];stats=state['observation']['world'];assert stats['source_terminate_discard'] and stats['native_gl_depth_control']==('portable'not in normal)
  name='resize.png'if row['name']=='resize'else str(i)+'-'+row['name']+'.png';assert hashlib.sha256(z.read('application/'+normal+'/app/'+name)).hexdigest()==row['image_sha256']
  if normal.startswith('solar'):assert state['solar'][2] and stats['source_gl_camera']==('portable'not in normal)
 assert b'PASS actual billboard application' in z.read('application/'+normal+'/actual-app.log')
 assert b'committed_bytes=0' in z.read('application/'+normal+'/app/runtime.log')
# Compare both real application paths through their unchanged captured ROI.
for ordinary,portable in [('normal','normal-portable'),('solar','solar-portable')]:
 a=read('application/'+ordinary+'/app/result.json');b=read('application/'+portable+'/app/result.json')
 for i,(r,s)in enumerate(zip(a['rows'],b['rows']),1):
  assert r['name']==s['name'] and r['state']['viewport']==s['state']['viewport']
  name='resize.png'if r['name']=='resize'else str(i)+'-'+r['name']+'.png';A=image('application/'+ordinary+'/app/'+name);B=image('application/'+portable+'/app/'+name)
  x,y,right,bottom=map(round,r['state']['viewport']);assert compare(A[y:bottom,x:right],B[y:bottom,x:right])[0]==0
# Literal altered-pixel and missing-inventory negatives exercise the actual comparators.
a=np.zeros((2,2,4),np.uint8);b=a.copy();b[0,0,0]=1;assert compare(a,b)[0]==1
try:inventory(heldout['cases'][:-1],heldout_names)
except AssertionError:pass
else:raise AssertionError('missing source control escaped')
failed=[r for r in rows if r['mode']=='portable'and r['pixels']]
print(json.dumps({'native_exact':16,'portable_frames_exact':24,'portable_maps_exact':7,'portable_strict_failures':failed,'legacy_negative_pixels':286,'actual_app_processes_pass':4},indent=2))
raise SystemExit(bool(failed))
