#!/usr/bin/env python3
"""Frozen actual source versus native application material interior samples."""
import hashlib,json,pathlib,sys
NAMES=['opaque-front','opaque-default-back','opaque-painted-back','alpha-front','alpha-painted-back','texture-blue','texture-red','texture-repeat','texture-default-back','texture-alpha','texture-shaded','texture-shaded-back','group-color','group-texture','group-reflected-texture','group-alpha','shaded-transparent-texture','shaded-back-texture','alpha-0.1','alpha-0.25','alpha-0.3','alpha-0.5','alpha-0.7','alpha-0.9']
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def check(source,native):
 s=json.loads((source/'source.json').read_text());side=source.parent/'material-shaded-side-source';extra=json.loads((side/'source.json').read_text());assert extra['source_pin']==PIN and extra['asset_sha256']==s['asset_sha256'];s['cases']+=extra['cases'];alpha=source.parent/'material-alpha-source';third=json.loads((alpha/'source.json').read_text());assert third['source_pin']==PIN and third['asset_sha256']==s['asset_sha256'];s['cases']+=third['cases'];r=json.loads((native/'report.json').read_text())
 assert s['passed'] is True and s['source_pin']==PIN and [c['name']for c in s['cases']]==NAMES
 assert hashlib.sha256((source/'quadrants.png').read_bytes()).hexdigest()==s['asset_sha256']
 assert r['passed'] is True and [f['name']for f in r['frames']]==[name+suffix for name in NAMES for suffix in['','-warm','-warm2']]
 assert (r['width'],r['height'])==(1280,800)and(r['ledger_committed'],r['ledger_reserved'])==(0,0)
 for i,case in enumerate(s['cases']):
  assert hashlib.sha256(((alpha if case['name'].startswith('alpha-0.') else side if case['name'].startswith('shaded-') else source)/(case['name']+'.png')).read_bytes()).hexdigest()==case['png_sha256']
  expected=bytes(case['rgba'])
  for f in r['frames'][i*3:i*3+3]:
   assert f['native'] is True and f['frame_present'] is True and f['route_reason'] is None and f['selection_count']==0
   data=(native/(f['name']+'.rgba')).read_bytes();assert len(data)==1280*800*4
   x,y=map(round,f['red_pixel']);l,t,w,h=f['rect'];assert l+3<x<l+w-3 and t+3<y<t+h-3
   assert all(data[4*(yy*1280+xx):4*(yy*1280+xx)+4]==expected for yy in range(y-2,y+3)for xx in range(x-2,x+3)),f['name']
  for k in ['catalog_builds','geometry_uploads','model_refreshes','matrix_bytes','texture_uploads','material_order_builds']:
   assert r['frames'][i*3+1][k]==r['frames'][i*3+2][k],(case['name'],k)
 assert r['frames'][-1]['texture_uploads']==1,'unchanged immutable image must be reused across material recipes'
 return {'passed':True,'source_cases':24,'native_cases':72,'pixels':1800,'scope':'Actual shared app update_ui/native Metal cached material sampled interiors; no full viewport, mip, edge, whole-image, user gesture or presentation parity claim'}
if __name__=='__main__':print(json.dumps(check(pathlib.Path(sys.argv[1]),pathlib.Path(sys.argv[2])),indent=2))
