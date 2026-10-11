#!/usr/bin/env python3
"""Strict retained actual Qt/RTX ownership, extent, warm and negative controls."""
import hashlib,json,re,sys
from pathlib import Path
import numpy as np
from PIL import Image
repo=Path(__file__).resolve().parents[1]
audit=Path(sys.argv[1])if len(sys.argv)>1 else repo/'docs/audits/annotation-fixed-boundary'
def load(path):return json.loads(path.read_text())
for name,sha in load(audit/'manifest.json').items():
 assert hashlib.sha256((audit/name).read_bytes()).hexdigest()==sha,'Retained artifact digest '+name
def image(path):
 value=np.array(Image.open(path).convert('RGBA'));assert value.shape==(600,800,4),str(path);return value
def pixels(a,b):assert a.shape==b.shape;return int(np.any(a!=b,axis=2).sum())
phase=[f'{axis}-{i:02}'for axis in ['top','bottom','left','right']for i in range(9)]
primitive=['horizontal-square','vertical-square','reversed','short','degenerate','cosmetic-zero','negative-origin','shallow-clipped','steep','narrow','fractional','triangle-clockwise','triangle-reversed','triangle-boundary']
def inventory(rows,names):assert [r['name']for r in rows]==names,'Mandatory source inventory/order'
records=[]
def check(source,native,names,density=1.,pose=None):
 document=load(source/'original.json');assert document['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9';inventory(document['cases'],names)
 assert {p.name for p in native.iterdir()if p.is_dir()}==set(names),'Mandatory native inventory'
 for row in document['cases']:
  name=row['name'];assert row['framebuffer']==[800,600]and row['samples']==0 and row['engine']==13;assert 'NVIDIA'in row['renderer']or'5090'in row['renderer']
  if names!=primitive:assert row['subpixel_bits']==8
  source_path=source/(name+'.png');assert hashlib.sha256(source_path.read_bytes()).hexdigest()==row['sha256']
  a=image(source_path);b=image(native/name/'native.png');assert pixels(a,b)==0,name
  state=load(native/name/'state.json');assert state['pixels_per_point']==density
  if pose is not None:assert state['pose_translation']==pose
  log=(native/name/'runtime.log').read_text();assert 'committed_bytes=0'in log;assert 'NVIDIA'in log
  found=re.search(r'Native annotation metrics: (\{.*\})',log);assert found;final=json.loads(found.group(1))
  assert final['geometry_uploads']==state['geometry_uploads']and final['query_uploads']==state['query_uploads']and final['geometry_cached_bytes']==state['cached_bytes']
  records.append(dict(name=name,density=density,pose=pose,pixels=0,final_ledger_zero=True,unchanged_warm_uploads=True))
 check_mutated=b.copy();check_mutated[0,0,0]^=1;assert pixels(a,check_mutated)==1,'Actual altered pixel negative'
 try:inventory(document['cases'][:-1],names)
 except AssertionError:pass
 else:raise AssertionError('Missing source case negative did not fail')
check(audit/'phase-source/capilla_quiroz_v17-iso/original',audit/'phase-native/native',phase)
check(audit/'dpr-source/capilla_quiroz_v17-iso/original',audit/'dpr-native/native',phase,1.25)
assert all(r['device_pixel_ratio']==1.25 and r['logical_extent']==[640,480]for r in load(audit/'dpr-source/capilla_quiroz_v17-iso/original/original.json')['cases'])
check(audit/'tie-source/capilla_quiroz_v17-iso/original',audit/'tie-native/native',[f'sloped-tie-{i:02}'for i in range(8)])
check(audit/'pose-source/capilla_quiroz_v17-iso/original',audit/'pose-native/native',phase,1.,[3.,-2.])
check(repo/'docs/audits/annotation-widget-square-strip/source/capilla_quiroz_v17-iso/original',audit/'primitives/native',primitive)
a=image(audit/'old-float-source/capilla_quiroz_v17-iso/original/top-boundary.png');b=image(audit/'old-float-native/native/top-boundary/native.png');assert pixels(a,b)==60,'Literal previous GPU float-edge failure'
source=repo/'docs/audits/annotation-widget-neutral-policy/capilla_quiroz_v17-iso/original';native=audit/'full-overlay'
a={p:image(source/f'annotations-{p}.png')for p in ['on','off','restored']};b={p:image(native/f'annotations-{p}.png')for p in a}
mask=np.any(a['on']!=a['off'],axis=2)|np.any(b['on']!=b['off'],axis=2);assert int(mask.sum())==3830 and int(np.any(a['on']!=b['on'],axis=2)[mask].sum())==0
whole={p:pixels(a[p],b[p])for p in a};assert whole==dict(on=1285,off=1297,restored=1285),'Retained whole scene failure changed'
print(json.dumps(dict(exact_cases=len(records),overlay_union=3830,overlay_mismatches=0,whole_scene_strict_failure=whole,negatives=['actual_changed_pixel','missing_named_case','old_gpu_float_edge_60_pixels']),indent=2))
