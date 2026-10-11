#!/usr/bin/env python3
"""Strict four-frame borrowed-Scene native Metal proof; no full viewport claim."""
import pathlib,json,sys
NAMES=('scene-source-global','scene-camera-only','scene-single-placement-move','scene-refused-alpha-retains-old')
def roi(data,rect,color):
 assert len(data)==64*64*4
 x,y,w,h=rect
 for py in range(y,y+h):
  for px in range(x,x+w):assert list(data[(py*64+px)*4:(py*64+px)*4+4])==color,(px,py)
def main(root):
 root=pathlib.Path(root);repo=pathlib.Path(__file__).resolve().parents[1]
 source=json.loads((repo/'docs/audits/metal-world-scene/source-catalog.json').read_text())
 assert source['passed'] is True and source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['name'] for r in source['rows']]==['root','A','B'] and all(r['same_mesh_as_root'] for r in source['rows'])
 assert all(r['paint']=={'color':[1,0,0]} for r in source['rows'])
 assert source['rows'][2]['matrix_columns'][12:16]==[4,2,0,1]
 report=json.loads((root/'report.json').read_text());assert report['passed'] is True
 assert report['cpu_bytes_before_warm']==report['cpu_bytes_after_warm'] and report['cpu_bytes_before_warm']>0
 assert report['catalog_builds']==1 and report['warm_geometry_uploads']==2 and report['draw_order_builds']==1
 assert report['single_model_upload_bytes']==64 and report['camera_only_upload_bytes']==0
 assert report['ledger_committed']==report['ledger_reserved']==0 and 'translucent' in report['alpha_refusal']
 assert {p.stem for p in root.glob('*.rgba')}==set(NAMES)
 data={n:(root/(n+'.rgba')).read_bytes() for n in NAMES}
 for n in NAMES:roi(data[n],(27,27,10,10),[247,0,0,255])
 for n in NAMES[2:]:roi(data[n],(48,30,5,5),[0,247,0,255])
 roi(data[NAMES[0]],(48,30,5,5),[0,0,255,255])
 assert data[NAMES[2]]==data[NAMES[3]]
 for name,bytes_,rect,color in [('blank',bytes([0,0,255,255])*4096,(27,27,10,10),[247,0,0,255]),('stale-model',data[NAMES[0]],(48,30,5,5),[0,247,0,255])]:
  try:roi(bytes_,rect,color)
  except AssertionError:pass
  else:raise AssertionError(name)
 print(json.dumps({'passed':True,'cases':list(NAMES),'matched_pixels':475,'negatives':['blank','stale-model'],'scope':'Borrowed actualScene producer/native Metal instance consumer, sampled interior/sourcecolor grouping, one64byte pose write and priorframe alpha refusal; host/edges/textures/sections/annotations/MSAA excluded'},indent=2))
if __name__=='__main__':main(sys.argv[1])
