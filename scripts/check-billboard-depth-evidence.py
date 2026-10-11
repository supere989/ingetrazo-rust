#!/usr/bin/env python3
import hashlib,io,json,pathlib,zipfile
from PIL import Image
import numpy as np
base=pathlib.Path(__file__).resolve().parents[1]/'docs/audits/billboard-depth-domain'
m=json.loads((base/'manifest.json').read_text());z=zipfile.ZipFile(base/'captures.zip')
assert hashlib.sha256((base/'captures.zip').read_bytes()).hexdigest()==m['archive_sha256']
assert set(z.namelist())==set(m['files'])
for n,h in m['files'].items():assert hashlib.sha256(z.read(n)).hexdigest()==h,n
expected={'billboard-mesh-placement-order','billboard-bitmap-empty-parent','billboard-bitmap-nested-reflection','solid','section','billboard-bitmap-hardcutout','billboard-bitmap-perspective','billboard-mesh-solid-perspective','glass-69','glass-70','cutout','hidden-line-cutout','missing-texture','offscreen-instance'}
rows=[];names=set()
def pixels(a,b):
 assert a.shape==b.shape
 return int(np.any(a!=b,axis=2).sum()),int(np.abs(a.astype(int)-b.astype(int)).max())
for source,native in [('source','native'),('source-alpha','native-alpha')]:
 r=json.loads(z.read(source+'/original.json'));assert r['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 for c in r['cases']:
  assert c['name'] not in names;names.add(c['name']);result=json.loads(z.read(native+'/'+c['name']+'/result.json'))
  assert result['source_commit']==m['source_commit'] and result['ledger_zero'] and result['refusal'] and result['samples']==4
  assert result['billboard_cold']['native_gl_depth_control']
  for x in c['images']+[{'phase':'depth','image':c['depth_image']}]:
   a=np.array(Image.open(io.BytesIO(z.read(source+'/'+x['image']))).convert('RGBA'));b=np.array(Image.open(io.BytesIO(z.read(native+'/'+c['name']+'/'+x['phase']+'.png'))).convert('RGBA'))
   assert a.shape==((2048,2048,4) if x['phase']=='depth' else (600,800,4))
   count,maximum=pixels(a,b);rows.append({'case':c['name'],'phase':x['phase'],'pixels':count,'max':maximum})
assert names==expected and len(rows)==56
assert rows==json.loads((base/'comparison.json').read_text())
a=np.zeros((2,2,4),np.uint8);b=a.copy();b[0,0,0]=1;assert pixels(a,b)[0]==1
app=json.loads(z.read('application/app/result.json'));assert app['source_sha']==m['source_commit'] and len(app['rows'])==8
assert b'PASS actual billboard application' in z.read('application/actual-app.log')
failed=[r for r in rows if r['pixels']];print(json.dumps({'exact':56-len(failed),'total':56,'failures':failed},indent=2))
raise SystemExit(bool(failed))
