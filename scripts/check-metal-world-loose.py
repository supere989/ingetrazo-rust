#!/usr/bin/env python3
"""Native actual-app sampled surfaces versus frozen actual Qt framebuffer RGBA."""
import json,pathlib,sys,hashlib
NAMES=['loose-initial','loose-warm','loose-geometry','loose-paint','loose-layer-hidden','loose-layer-restored','loose-topology-removed']
NATIVE_NAMES=NAMES[:2]+['loose-warm2']+NAMES[2:]
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def check(source,native):
 s=json.loads((source/'source.json').read_text());r=json.loads((native/'report.json').read_text())
 assert s['passed'] is True and s['source_pin']==PIN
 assert [c['name']for c in s['cases']]==NAMES
 assert [f['name']for f in r['frames']]==NATIVE_NAMES
 assert r['passed'] is True and (r['width'],r['height'])==(1280,800)
 assert (r['ledger_committed'],r['ledger_reserved'])==(0,0)
 expected={c['name']:bytes(c['rgba'])for c in s['cases']}
 expected['loose-warm2']=expected['loose-warm'] # unchanged source recipe, retained actual framebuffer evidence
 for c in s['cases']:assert hashlib.sha256((source/(c['name']+'.png')).read_bytes()).hexdigest()==c['png_sha256']
 assert expected[NAMES[0]]==expected[NAMES[1]]==expected[NAMES[2]]
 red=expected[NAMES[0]];yellow=expected[NAMES[3]]
 assert 0<red[0]<=255 and red[1:]==bytes([0,0,255])
 assert yellow[0]==yellow[1]>0 and yellow[2:]==bytes([0,255])
 assert expected[NAMES[4]]==expected[NAMES[6]]==bytes([0,0,255,255])
 assert expected[NAMES[5]]==yellow
 def patch(image,point,rect):
  x,y=map(round,point);l,t,w,h=rect;assert l+3<x<l+w-3 and t+3<y<t+h-3
  return [image[4*(yy*1280+xx):4*(yy*1280+xx)+4]for yy in range(y-2,y+3)for xx in range(x-2,x+3)]
 frames=r['frames']
 for f in frames:
  assert f['native'] is True and f['selection_count']==0
  image=(native/(f['name']+'.rgba')).read_bytes();assert len(image)==1280*800*4
  assert all(p==expected[f['name']]for p in patch(image,f['red_pixel'],f['rect'])),f['name']
  if f['name']=='loose-geometry':assert all(p==bytes([0,0,255,255])for p in patch(image,f['old_pixel'],f['rect'])),'stale old topology'
 for k in ['catalog_builds','geometry_uploads','model_refreshes']:assert frames[0][k]==frames[1][k],k
 for k in ['catalog_builds','geometry_uploads','model_refreshes','matrix_bytes']:assert frames[1][k]==frames[2][k],k
 for previous,current in zip(frames[2:],frames[3:]):assert current['catalog_builds']>previous['catalog_builds'] and current['geometry_uploads']>previous['geometry_uploads']
 return {'passed':True,'native_cases':len(NATIVE_NAMES),'source_cases':len(NAMES),'pixels':len(NATIVE_NAMES)*25+25,'scope':'Actual shared native app consumer; exact Qt framebuffer interior samples and cache invalidation, not whole-image/OS gesture/presentation parity'}
if __name__=='__main__':print(json.dumps(check(pathlib.Path(sys.argv[1]),pathlib.Path(sys.argv[2])),indent=2))
