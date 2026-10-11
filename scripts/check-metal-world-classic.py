#!/usr/bin/env python3
"""Actual native app classic world-shading, winding and layer sampled protocol."""
import json,pathlib,sys,hashlib
NAMES=['classic-initial','classic-warm','classic-pose','classic-paint','classic-layer-hidden','classic-layer-restored','classic-reflected','classic-group-hidden','classic-group-restored']
NATIVE=NAMES[:2]+['classic-warm2']+NAMES[2:]
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def check(source,native):
 s=json.loads((source/'source.json').read_text());r=json.loads((native/'report.json').read_text())
 assert s['passed'] is True and s['source_pin']==PIN and [c['name']for c in s['cases']]==NAMES
 assert r['passed'] is True and [f['name']for f in r['frames']]==NATIVE
 assert (r['width'],r['height'])==(1280,800)and(r['ledger_committed'],r['ledger_reserved'])==(0,0)
 expected={c['name']:bytes(c['rgba'])for c in s['cases']};expected['classic-warm2']=expected['classic-warm']
 for c in s['cases']:assert hashlib.sha256((source/(c['name']+'.png')).read_bytes()).hexdigest()==c['png_sha256']
 assert expected['classic-initial']==bytes([159,0,0,255])and expected['classic-pose']==bytes([207,0,0,255])
 assert expected['classic-layer-hidden']==expected['classic-layer-restored']==expected['classic-paint']==bytes([207,207,0,255])
 assert expected['classic-reflected']==expected['classic-group-restored']==bytes([158,178,199,255])
 assert expected['classic-group-hidden']==bytes([0,0,255,255])
 for f in r['frames']:
  assert f['native'] is True and f['selection_count']==0
  data=(native/(f['name']+'.rgba')).read_bytes();assert len(data)==1280*800*4
  x,y=map(round,f['red_pixel']);l,t,w,h=f['rect'];assert l+3<x<l+w-3 and t+3<y<t+h-3
  pixels=[data[4*(yy*1280+xx):4*(yy*1280+xx)+4]for yy in range(y-2,y+3)for xx in range(x-2,x+3)]
  assert all(p==expected[f['name']]for p in pixels),f['name']
 frames=r['frames']
 for k in ['catalog_builds','geometry_uploads','model_refreshes','matrix_bytes']:assert frames[1][k]==frames[2][k],k
 assert frames[3]['catalog_builds']==frames[2]['catalog_builds']+1 and frames[3]['geometry_uploads']==frames[2]['geometry_uploads']+1
 assert frames[3]['matrix_bytes']==frames[2]['matrix_bytes'],'classic baked world data must not double-transform model matrices'
 assert frames[7]['catalog_builds']>frames[6]['catalog_builds'] and frames[7]['geometry_uploads']>frames[6]['geometry_uploads']
 return {'passed':True,'source_cases':9,'native_cases':10,'pixels':250,'scope':'Actual native app sampled classic own-mesh world shading/baked reflection/face-vs-group layer routing, no whole-image/OS gesture/presentation claim'}
if __name__=='__main__':print(json.dumps(check(pathlib.Path(sys.argv[1]),pathlib.Path(sys.argv[2])),indent=2))
