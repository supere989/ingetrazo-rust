#!/usr/bin/env python3
"""Read-only frozen source/consumer/image/resource checks; no GPU or extraction."""
import hashlib,json,zipfile
from pathlib import Path
from io import BytesIO
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[1]/'docs/audits/texture-upload-batching'
manifest=json.loads((root/'manifest.json').read_text());raw=(root/'captures.zip').read_bytes()
assert hashlib.sha256(raw).hexdigest()==manifest['archive_sha256']
with zipfile.ZipFile(BytesIO(raw))as z:
 assert len(z.namelist())==len(set(z.namelist())) and set(z.namelist())==set(manifest['files'])
 records={name:z.read(name)for name in z.namelist()}
for name,data in records.items():assert hashlib.sha256(data).hexdigest()==manifest['files'][name],name
j=lambda name:json.loads(records[name])
def rgba(name):
 image=Image.open(BytesIO(records[name])).convert('RGBA');assert image.size==(800,600),name;return np.asarray(image)
def diff(a,b):
 assert a.shape==b.shape
 return {'pixels':int(np.any(a!=b,axis=2).sum()),'max_channel':int(np.abs(a.astype(np.int16)-b.astype(np.int16)).max())}
def inventory(report):assert [row['case']for row in report['cases']]==['near','far']
original=j('original/original.json');confirmed=j('original-msaa/original.json')
assert original['source_pin']==confirmed['source_pin']==manifest['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert hashlib.sha256(records['original/batch.igz']).hexdigest()==original['igz_sha256']
for report in [original,confirmed]:
 inventory(report);assert len(report['assets'])==5
 for row in report['cases']:
  assert 'NVIDIA GeForce RTX 5090' in row['gl_info']['renderer']
  assert [f['levels']for f in row['filters']]==[6,7,9,8,4]
  assert [f['min']for f in row['filters']]==[9987]*4+[9729]
  assert [f['mag']for f in row['filters']]==[9729]*5
  assert [f['cutout']for f in row['filters']]==[False]*4+[True]
for row in confirmed['cases']:assert row['scene_fbo_samples']==4
for report,prefix in [(original,'original'),(confirmed,'original-msaa')]:
 for asset in report['assets']:assert hashlib.sha256(records[prefix+'/'+asset['name']]).hexdigest()==asset['sha256']
final=j('native-final/native.json');prior=j('native-prior-consumer/native.json');inventory(final);inventory(prior)
assert final['source']==manifest['renderer_source'] and prior['source']==manifest['prior_consumer_observer']
for prefix,report in [('native-final',final),('native-prior-consumer',prior)]:
 transport=j(prefix+'/transport.json');assert transport['returncode']==0 and transport['source']==report['source'] and len(report['source'])==40
 assert transport['reference_sha256']==hashlib.sha256(records['original/original.json']).hexdigest()
 assert 'NVIDIA GeForce RTX 5090' in records[prefix+'/runtime.log'].decode() and 'TYPE_4' in records[prefix+'/runtime.log'].decode()
 assert report['ledger_final']==0 and report['old_frame_retained'] and report['retry_exact'] and 'budget' in report['pressure_error']
 for name in ['image_transfer_submissions','image_upload_calls','image_bytes','image_mips_generated','transfer_submissions','upload_calls','upload_staging_maps']:
  assert report['cold'][name]==report['warm'][name],(prefix,name)
 assert report['cold']['image_upload_calls']==5 and report['cold']['image_mips_generated']==26 and report['cold']['image_bytes']==315456
 assert report['cold']['staging_peak']<=8*1024*1024
for a,b in zip(original['cases'],final['cases']):assert a['samples']==b['samples'] and len(a['samples'])==15
assert prior['cold']['image_transfer_submissions']==5 and final['cold']['image_transfer_submissions']==1
large=final['large'];assert large['extent']==[2048,2176] and large['raw_bytes']==17825792 and large['all_base_bytes_exact']
assert large['cold']['image_transfer_submissions']==3 and large['cold']['image_bytes']==17825792 and large['cold']['image_mips_generated']==11
assert large['cold']['staging_peak']==8388608 and large['cold']['upload_staging_maps']==1 and large['cold']['upload_staging_unmaps']==0
assert large['sampled_allocation_bytes']==24424448 and large['warm_uploads']==0 and large['completed_retirement']==1 and large['ledger_final']==0 and 'budget' in large['actual_budget_refusal']
comparisons=[]
for name in ['near','far']:
 source=rgba('original/'+name+'.png');confirmed_source=rgba('original-msaa/'+name+'.png')
 assert diff(source,confirmed_source)['pixels']==0
 for prefix in ['native-final','native-prior-consumer']:
  result=diff(source,rgba(prefix+'/'+name+'.png'));assert result['pixels']==0
  comparisons.append({'case':name,'consumer':prefix,**result})
 historical=diff(source,rgba('historical-single-sample/'+name+'.png'))
 assert historical['pixels']=={'near':1902,'far':573}[name]
 comparisons.append({'case':name,'consumer':'historical-single-sample','scope':'intentional sample-count mismatch retained',**historical})
# Literal pixel and inventory mutations use the exact same admission/compare helpers.
a=rgba('native-final/near.png');b=a.copy();b[100,100,0]^=1;assert diff(a,b)['pixels']==1
try:inventory({'cases':[{'case':'near'}]})
except AssertionError:pass
else:raise AssertionError('Missing-case mutation was admitted')
result={'passed':True,'comparisons':comparisons,'source_samples_exact':30,'old_image_submissions':5,'new_image_submissions':1,'old_staging_bytes':prior['cold']['staging_peak'],'new_staging_bytes':final['cold']['staging_peak'],'cold_wall_ms':{'old':prior['cold_ms'],'new':final['cold_ms'],'scope':'single comparable full World.set_scene measurement; not a throughput benchmark'},'large':large,'scope':'actual direct production Vulkan World consumer; power-of-two mip/control images exact, known odd-size source-mip differences and broader streaming remain open'}
expected=root/'result.json'
if expected.exists():assert json.loads(expected.read_text())==result
print(json.dumps(result,indent=2))
