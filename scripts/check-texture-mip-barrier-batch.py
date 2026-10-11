#!/usr/bin/env python3
"""Read-only authentication and exact retained GPU comparisons; no GPU execution."""
import hashlib,io,json,pathlib,zipfile
import numpy as np
from PIL import Image
ROOT=pathlib.Path(__file__).resolve().parents[1]
P=ROOT/'docs/audits/texture-mip-barrier-batch'
def sha(b):return hashlib.sha256(b).hexdigest()
def compare(a,b):
 assert len(a)==len(b) and len(a)%4==0
 x=np.frombuffer(a,np.uint8).reshape(-1,4);y=np.frombuffer(b,np.uint8).reshape(-1,4)
 return int(np.any(x!=y,axis=1).sum()),int(np.abs(x.astype(np.int16)-y).max(initial=0))
def main():
 m=json.loads((P/'manifest.json').read_text());assert sha((P/'captures.zip').read_bytes())==m['zip_sha256']
 z=zipfile.ZipFile(P/'captures.zip');data={}
 assert set(z.namelist())=={r['payload'] for r in m['records']}
 for r in m['records']:
  b=z.read(r['payload']);assert len(b)==r['bytes'] and sha(b)==r['sha256'];assert r['name'] not in data;data[r['name']]=b
 for path,h in m['references'].items():assert sha((ROOT/path).read_bytes())==h
 def js(n):return json.loads(data[n])
 baseline=js('baseline-mips/native.json');candidate=js('candidate-mips/native.json');single=js('final-single/native.json')
 assert baseline['cases']==candidate['cases'] and len(candidate['cases'])==4
 original=zipfile.ZipFile(ROOT/'docs/audits/texture-mip-source-diagnosis/assets.zip')
 residuals=[];levels=0
 for c in candidate['cases']:
  stem=pathlib.Path(c['asset']).stem;total=maximum=0
  for row in c['levels']:
   n=f"{stem}-level{row['level']}.rgba";a=data['candidate-mips/'+n];assert a==data['baseline-mips/'+n]
   count,error=compare(a,original.read('original/'+n));assert (count,error,len(a))==(row['different_pixels'],row['max_channel_error'],row['bytes'])
   if row['level']==0:assert count==0
   total+=count;maximum=max(maximum,error);levels+=1
  residuals.append([total,maximum])
 assert levels==38 and residuals==[[1020,9],[28094,10],[5596,72],[4876,58]]
 assert baseline['resources']['mip_barrier_commands']==72 and candidate['resources']['mip_barrier_commands']==38
 assert {k:v for k,v in baseline['resources'].items() if k!='mip_barrier_commands'}=={k:v for k,v in candidate['resources'].items() if k!='mip_barrier_commands'}
 assert single['source']==m['final_source'] and single['cases']==candidate['cases']
 for case in single['cases']:
  for row in case['levels']:
   n=f"{pathlib.Path(case['asset']).stem}-level{row['level']}.rgba";assert data['final-single/'+n]==data['candidate-mips/'+n]
 assert single['resources']['mip_barrier_commands']==38
 assert all(q['ledger_final']==0 for q in (baseline,candidate,single))
 world=js('world/native.json');assert world['old_frame_retained'] and world['retry_exact'] and world['ledger_final']==0
 assert world['cold']['mip_barrier_commands']==27 and world['cold']['image_mips_generated']==26
 for k in ('image_bytes','image_upload_calls','image_mips_generated','mip_barrier_commands','transfer_submissions'):assert world['warm'][k]==world['cold'][k]
 large=world['large'];assert large['all_base_bytes_exact'] and large['warm_uploads']==0 and large['completed_retirement']==1 and large['ledger_final']==0 and large['actual_budget_refusal']
 assert large['cold']['mip_barrier_commands']==12 and large['cold']['transfer_submissions']==3 and large['cold']['staging_peak']<=8*1024*1024
 source=zipfile.ZipFile(ROOT/'docs/audits/texture-upload-batching/captures.zip')
 for name in ('near','far'):
  a=np.asarray(Image.open(io.BytesIO(data[f'world/{name}.png'])).convert('RGBA'));b=np.asarray(Image.open(io.BytesIO(source.read(f'original-msaa/{name}.png'))).convert('RGBA'))
  assert a.shape==b.shape==(600,800,4) and np.array_equal(a,b)
 for prefix in ('baseline-mips','candidate-mips','final-single','world'):assert 'NVIDIA GeForce RTX 5090' in data[prefix+'/runtime.log'].decode()
 # Actual comparator negatives: changed pixel and truncated mip must refuse equality.
 raw=data['candidate-mips/legacy-dib-22-level0.rgba'];mut=bytearray(raw);mut[0]^=1;assert compare(raw,mut)==(1,1)
 try:compare(raw,raw[:-1])
 except AssertionError:pass
 else:raise AssertionError('truncated negative accepted')
 return dict(verdict='PASS',raw_levels_equal=38,world_frames_exact=2,final_single_levels=38,barrier_commands=[72,38],odd_mip_source_residuals=residuals,negative_controls=2,scope='barrier reduction; odd mip source parity remains FAIL; no timing claim')
if __name__=='__main__':print(json.dumps(main(),indent=2))
