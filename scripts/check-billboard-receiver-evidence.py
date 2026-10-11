#!/usr/bin/env python3
"""Strict retained whole-image comparator; exit 1 preserves any parity failure."""
import argparse,hashlib,io,json,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('directory');parser.add_argument('--output');args=parser.parse_args()
d=Path(args.directory);manifest=json.loads((d/'manifest.json').read_text());raw=(d/'captures.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==manifest['archive_sha256']
z=zipfile.ZipFile(io.BytesIO(raw));assert set(z.namelist())==set(manifest['files'])
for name,record in manifest['files'].items():
 data=z.read(name);assert len(data)==record['bytes'] and hashlib.sha256(data).hexdigest()==record['sha256'],name
def read_json(name):return json.loads(z.read(name))
def pixels(name):
 a=np.asarray(Image.open(io.BytesIO(z.read(name))).convert('RGBA'));assert a.shape==(600,800,4),name;return a
names=['billboard-bitmap','billboard-mesh','billboard-bitmap-perspective','billboard-bitmap-oblique','billboard-bitmap-nested-reflection','billboard-mesh-solid-perspective','billboard-mesh-uv-transform','billboard-mesh-mixed-order','billboard-bitmap-missing','billboard-bitmap-hardcutout','billboard-mesh-placement-order','billboard-bitmap-empty-parent']
rows=[];seen=[]
for bundle in ['base-heldout','hardcutout','order-nested']:
 original=read_json('source/'+bundle+'/original.json');assert original['source_pin']==manifest['source_pin']
 transport=read_json('native/'+bundle+'/transport.json');assert len(transport['binary_sha256'])==64
 for c in original['cases']:
  name=c['name'];seen.append(name);assert c['receiver_samples']==4 and c['scene_format_samples']==4 and 'NVIDIA' in c['gl']['vendor']
  prefix='native/'+bundle+'/'+name+'/';result=read_json(prefix+'result.json');assert result['ledger_zero'] and result['samples']==4 and result['refusal'];assert len(result['source_commit'])==40
  log=z.read(prefix+'runtime.log').decode();assert 'NVIDIA' in log
  for phase in ['on','off','restored']:
   a=pixels('source/'+bundle+'/'+name+'-'+phase+'.png');b=pixels(prefix+phase+'.png');diff=np.any(a!=b,axis=2)
   rows.append(dict(case=name,phase=phase,pixels=int(diff.sum()),max_channel=int(np.abs(a.astype(np.int16)-b).max())))
assert seen==names,(seen,names)
app=read_json('application/app/result.json');assert app['passed'] and len(app['source_sha'])==40
assert [r['name'] for r in app['rows']]==['camera','original','hide','restore','refuse','retry','original','resize']
worlds=[r['state']['observation']['world'] for r in app['rows']];assert worlds[0]['bc7_uploads']==1 and worlds[0]['image_decodes']==1 and worlds[0]['sampled_entries']==1
assert worlds[0]['image_uploads']==worlds[1]['image_uploads'];assert worlds[3]['bc7_uploads']==worlds[4]['bc7_uploads']==worlds[5]['bc7_uploads']==worlds[-1]['bc7_uploads']==2
log=z.read('application/app/runtime.log').decode();assert 'BILLBOARD_APPLICATION_PASS' in log and 'present_fences_enabled=true; committed_bytes=0' in log
result=dict(rows=rows,whole_frame_exact=sum(r['pixels']==0 for r in rows),whole_frame_total=len(rows),application_pass=True,strict_parity_pass=all(r['pixels']==0 for r in rows),scope=manifest['scope'])
if args.output:Path(args.output).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2));raise SystemExit(0 if result['strict_parity_pass'] else 1)
