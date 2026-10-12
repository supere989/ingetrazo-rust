#!/usr/bin/env python3
"""Diagnostic-only strict source pixels/sample/depth evidence; never admission."""
import argparse,copy,hashlib,importlib.util,json,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def load_module(name,file):
 s=importlib.util.spec_from_file_location(name,ROOT/'scripts'/file);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
strict=load_module('strict_resolve','check-metal-section-resolve.py')
def authenticate():
 m=json.loads((ROOT/'docs/audits/intel-metal-reflected-samples/source-bindings.json').read_text())
 for path,row in m['files'].items():
  data=(ROOT/path).read_bytes();assert len(data)==row['size'] and hashlib.sha256(data).hexdigest()==row['sha256'],path
 return m
def samples(root,case,source,depth24):
 n=json.loads((root/(case+'-samples.json')).read_text());rows=source['pixels'];assert n['selected_source_sample']==0
 if depth24:assert n['transport']['depth']['format']=='MTLPixelFormat(255)' and n['transport']['depth']['samples']==4
 assert n['points_top_origin'][:len(rows)]==[r['point_top_origin']for r in rows]
 assert len(n['sample_rgba_bits'])==len(n['sample_depth_bits'])==16
 assert all(type(v)is int and 0<=v<=0xffffffff for row in n['sample_rgba_bits']for v in row)
 assert all(type(v)is int and 0<=v<=0xffffffff for v in n['sample_depth_bits'])
 expected=[v for r in rows for v in r['sample_float_bits']]
 actual=n['sample_rgba_bits'][:len(expected)]
 depth=[v for r in rows for v in r.get('sample_depth_bits',[])]
 actual_depth=n['sample_depth_bits'][:len(depth)]
 return {'case':case,'source_pixels':len(rows),'rgba_bits_equal':evidence_equal(actual,expected),'depth_bits_equal':evidence_equal(actual_depth,depth) if depth else None,'actual_rgba_bits':actual,'source_rgba_bits':expected,'actual_depth_bits':actual_depth,'source_depth_bits':depth,'transport':n['transport']}
def evidence_equal(a,b):
 return a==b
def require_same(a,b):
 assert evidence_equal(a,b),'strict evidence mismatch'
def negatives(source_image,source_samples,source_depth):
 # Baselines are the frozen original values, not already-failing candidate data.
 count=0
 for before,after in [(source_image,b''.join(source_image[i:i+3200]for i in range(len(source_image)-3200,-1,-3200))),
                      (source_samples,[*source_samples[1:4],source_samples[0],*source_samples[4:]])]:
  require_same(before,before)
  try:require_same(before,after)
  except AssertionError:count+=1
  else:raise AssertionError('ineffective negative control')
 for index in (0,1,len(source_depth)-1):
  altered=source_depth.copy();altered[index]^=1;require_same(source_depth,source_depth)
  try:require_same(source_depth,altered)
  except AssertionError:count+=1
  else:raise AssertionError('depth corruption accepted')
 return count

def main():
 p=argparse.ArgumentParser();p.add_argument('native',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 result={'ok':False,'production_admitted':False,'scope':'Diagnostic candidate versus distinct frozen Mesa image/sample/depth and RTX selector evidence; no valid Intel CGL oracle inferred'}
 try:
  result['source_bindings']=authenticate()
  report=json.loads((a.native/'report.json').read_text());assert report['reflected_diagnostic'] is True and report['production_mode'] is False
  assert report['sample_diagnostics']['strict_nvidia_reference_policy']['admitted'] is False
  defaults=report['sample_diagnostics']['default_positions_top_origin'];source=report['sample_diagnostics']['source_positions_top_origin'];assert all(d[0]==s[0]and 1-d[1]==s[1]for d,s in zip(defaults,source)) and len(defaults)==len(source)==4
  control=report['cases'][0]['diagnostic_controls'];assert control['held_pixels_preserved'] is True and control['foreign_owner_rejected'] is True and control['warm_mode_rejected'] is True and type(control['budget_refusal'])is str and control['budget_refusal']
  result['images']=strict.validate(a.native,ROOT/'docs/audits/section-original-gl')
  with zipfile.ZipFile(ROOT/'docs/audits/section-sample-observation/records.zip')as z:closed=json.loads(z.read('mesa/report.json'))
  with zipfile.ZipFile(ROOT/'docs/audits/section-iso-samples/source.zip')as z:iso=json.loads(z.read('report.json'))
  depth24=report.get('depth24_diagnostic',False);assert type(depth24)is bool
  if depth24:assert report['depth24_capability'] is True
  result['depth24_diagnostic']=depth24
  result['samples']=[samples(a.native,'closed',closed,depth24),samples(a.native,'closed-iso',iso,depth24)]
  image=strict.strict.image(ROOT/'docs/audits/section-original-gl/closed-iso.png')
  expected_samples=[v for r in closed['pixels']for v in r['sample_float_bits']];expected_depth=[v for r in iso['pixels']for v in r['sample_depth_bits']]
  result['negative_controls']=negatives(image,expected_samples,expected_depth)
  result['ok']=result['images']['passed'] and all(r['rgba_bits_equal']and r['depth_bits_equal']is not False for r in result['samples'])
 except Exception as e:result['error']=str(e)
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'ok':result['ok'],'scope':result['scope']}));raise SystemExit(0 if result['ok']else 1)
if __name__=='__main__':main()
