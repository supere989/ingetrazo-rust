#!/usr/bin/env python3
"""Literal successful GeoTIFF observations; separate atomic correction of source fault."""
import copy,json,math,sys
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
NAMES=['fresh-datum','existing-rotated-datum','gui-reduced','chooser-cancel','decode-failure-datum','nested-global-image']
DECODES=['strip-raw-rgba8','strip-deflate-predictor8','strip-packbits-grayalpha8','strip-lzw-rgb8','tile-raw-partial','tile-deflate-reduced','tile-lzw-reduced','old-deflate-odd-carry','bigtiff-be-planar16','classic-planar8','whitezero16','rgba16-be-predictor','geographic-affine-point','citation-utm','jpeg-tables-cmyk','jpeg-rgb']
def exact(a,b,p=''):
 assert type(a)is type(b),(p,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(p,'keys')
  for k in a:exact(a[k],b[k],p+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(p,'length')
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,p+'/'+str(i))
 else:
  assert a==b,(p,a,b)
  if isinstance(a,float):
   assert math.isfinite(a)and math.isfinite(b)
   if a==0:assert math.copysign(1,a)==math.copysign(1,b),(p,'signedzero')
def validate(a,b):
 assert a['source_pin']==PIN
 assert [c['name']for c in a['cases']]==NAMES
 assert [r['case']for r in a['decodes']]==DECODES
 inventory=[(c['name'],p)for c in a['cases']for p in c['phases']]
 assert len(inventory)==21 and [(r['case'],r['phase'])for r in a['states']]==inventory
 assert [r['case']for r in a['resources']]==['fresh-datum','existing-rotated-datum','gui-reduced','nested-global-image']
 assert len(a['source_defects'])==1
 defect=a['source_defects'][0];assert defect['case']=='decode-failure-datum' and defect['kind']=='datum-published-before-failed-decode' and defect['exception']['type']=='zlib.error'
 assert 'read_rgba' in defect['exception']['traceback']
 # Verify the native correction independently, before comparing literal remaining fields.
 def row(v,phase):return next(r for r in v['states']if r['case']=='decode-failure-datum'and r['phase']==phase)
 af=row(a,'failed');bf=row(b,'failed');before=row(b,'before')
 assert af['outcome']=='source-decoder-exception' and bf['outcome']=='native-atomic-refusal'
 exact(af['scene']['datum'],defect['datum']);assert af['scene']['datum']is not None
 for key in ['scene','camera','undo','redo']:exact(before[key],bf[key],'native-atomic-refusal/'+key)
 expected=copy.deepcopy(a);observed=copy.deepcopy(b)
 # Only this declared source fault diverges. Raw reports and original source stay intact.
 ea=row(expected,'failed');eb=row(observed,'failed')
 ea['scene']['datum']=row(a,'before')['scene']['datum'];ea['outcome']='native-atomic-refusal'
 exact(expected,observed)
def file(root,name):
 p=(root/name).resolve();assert p.is_relative_to(root),name;return p
src=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve();report=Path(sys.argv[3]);result={'ok':False,'strict_all_source_states':False,'scope':'20 literal successful/cancel states and one separately accepted native atomic correction; original leaked datum and uncaught zlib error retained'}
try:
 a=json.loads((src/'original-geotiff.json').read_text());b=json.loads((native/'native-geotiff.json').read_text());validate(a,b)
 payloads=0
 for section,key in [('decodes','rgba'),('resources','file'),('resources','rgba')]:
  for r in a[section]:assert file(src,r[key]).read_bytes()==file(native,r[key]).read_bytes(),(section,r['case'],key);payloads+=1
 readers=json.loads((native/'reopens-geotiff.json').read_text());assert [r['case']for r in readers]==[c['name']for c in a['cases']if c['saved']]
 for r in readers:exact(r['original'],r['native'])
 guards=json.loads((native/'guards-geotiff.json').read_text())
 for k in ['actual_ingress','atomic_failure','chooser_cancel','streaming_admission','tiny_limit_refusal']:assert guards.get(k)is True,('guard',k)
 probes=[]
 for k in ['undo','redo']:
  c=copy.deepcopy(b);c['states'][0][k]+=1;probes.append(c)
 c=copy.deepcopy(b);c['states'].pop();probes.append(c)
 c=copy.deepcopy(b);c['states'][0]['scene']['geometry']={'corruption':True};probes.append(c)
 c=copy.deepcopy(b);c['decodes'][0]['factor']+=1;probes.append(c)
 c=copy.deepcopy(b);next(r for r in c['states']if r['case']=='fresh-datum'and r['phase']=='import')['scene']['images'][0]['texture']['path']['encoded_sha256']='0'*64;probes.append(c)
 for field in ['datum','undo']:
  c=copy.deepcopy(b);r=next(r for r in c['states']if r['case']=='decode-failure-datum'and r['phase']=='failed')
  if field=='datum':r['scene']['datum']=a['source_defects'][0]['datum']
  else:r['undo']+=1
  probes.append(c)
 for c in probes:
  try:validate(a,c)
  except (AssertionError,KeyError,StopIteration):pass
  else:raise AssertionError('negative accepted')
 result.update(ok=True,cases=6,states=21,literal_states=20,native_correction_states=1,native_correction_ok=True,decodes=16,stored_resources=4,payloads=payloads,reader_pairs=len(readers),negative_controls=len(probes))
except Exception as e:result['error']=repr(e)
report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));sys.exit(0 if result['ok']else 1)
