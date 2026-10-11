#!/usr/bin/env python3
"""Strict actual original centre bytes for frozen source-global grouping."""
import pathlib,json,hashlib,sys
NAMES=('source-global-order','camera-only-global-order','independent-last-prototype-negative')
SOURCE_NAMES=('source-global-prototype-order','ungrouped-source-negative')
def compare(pixels,expected):
 assert len(pixels)==64*64*4 and len(expected)==100
 for i,(y,x) in enumerate(( (y,x) for y in range(27,37) for x in range(27,37))):
  actual=list(pixels[(y*64+x)*4:(y*64+x)*4+4]);assert actual==expected[i],(x,y,actual,expected[i])
def main(root):
 root=pathlib.Path(root);repo=pathlib.Path(__file__).resolve().parents[1]
 source=repo/'docs/audits/metal-world-placements/source-global-order'
 ref=json.loads((source/'review.json').read_text());assert ref['passed'] is True
 assert ref['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert tuple(c['name'] for c in ref['cases'])==SOURCE_NAMES
 for c in ref['cases']:
  assert c['roi']==[195,195,10,10] and len(c['pixels'])==100
  assert hashlib.sha256((source/(c['name']+'.png')).read_bytes()).hexdigest()==c['png_sha256']
 assert ref['cases'][0]['instance_group_names']==[['A1','A2'],['B']]
 assert ref['cases'][1]['instance_group_names']==[]
 assert set(map(tuple,ref['cases'][0]['pixels']))=={(247,0,0,255)}
 assert set(map(tuple,ref['cases'][1]['pixels']))=={(0,247,0,255)}
 report=json.loads((root/'report.json').read_text());assert report['passed'] is True
 assert report['grouped_runs']==2 and report['independent_runs']==3
 assert report['geometry_uploads_before_warm']==report['geometry_uploads_after_warm']==2
 assert report['order_builds_before_warm']==report['order_builds_after_warm']==1
 assert report['matrix_bytes_before_warm']==report['matrix_bytes_after_warm']==192
 assert report['ledger_committed']==report['ledger_reserved']==0
 assert {p.stem for p in root.glob('*.rgba')}==set(NAMES)
 data={n:(root/(n+'.rgba')).read_bytes() for n in NAMES}
 for n in NAMES:compare(data[n],ref['cases'][1 if n==NAMES[2] else 0]['pixels'])
 for n,b in [('blank',bytes([0,0,255,255])*4096),('independent-as-shared',data[NAMES[2]])]:
  try:compare(b,ref['cases'][0]['pixels'])
  except AssertionError:pass
  else:raise AssertionError(n)
 print(json.dumps({'passed':True,'matched_source_pixels':300,'cases':list(NAMES),'negatives':['blank','independent-as-shared'],'scope':'actual source global prototype order, native3GPU centreROIs, cold/warm model/camera counters and ledger; edges/fullscene/host/performance excluded'},indent=2))
if __name__=='__main__':main(sys.argv[1])
