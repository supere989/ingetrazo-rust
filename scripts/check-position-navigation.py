#!/usr/bin/env python3
"""Strict named actual-source/controller parity; image visibility is separate."""
import copy,json,math,sys
from pathlib import Path
NAMES=['front-empty','right-empty','top-empty','iso-ground','front-locked-image','face-before-image','position-drag','position-cancel','position-six-pixels','loose-orphan','visible-edge-bounds','draw-hidden-edge-bounds','hidden-layer-bounds','hidden-group-bounds','group-orphan-bounds','hidden-nested-descendant-bounds']
def finite(v):
 if isinstance(v,list):return all(finite(x)for x in v)
 return isinstance(v,(int,float))and math.isfinite(v)
def compare(source,native):
 a=source['cases'];b=native['cases'];assert [r['name']for r in a]==[r['name']for r in b]==NAMES,'mandatory named inventory/order'
 rows=[]
 for x,y in zip(a,b):
  for key in ['point','after_eye','after_forward','before_eye','preview']:assert finite(x[key])and finite(y[key]),(x['name'],key,'nonfinite')
  assert x['point']==y['point'],(x['name'],'source float cursor point differs')
  assert x['snap'].replace('_','').lower()==y['snap'].replace('_','').lower(),(x['name'],'snap kind')
  assert x['cancelled']==y['cancelled'] and x['undo']==y['undo']==0,(x['name'],'history/cancel')
  expected='Some(Position)'if x['cancelled']else'Some(Look)';assert y['mode']==expected,(x['name'],'handoff')
  assert x['mode']==('PositionCameraTool'if x['cancelled']else'LookAroundTool'),(x['name'],'original handoff')
  errors={key:max(abs(v-w)for v,w in zip(x[key],y[key]))for key in ['after_eye','after_forward','before_eye']}
  assert max(errors.values())<=2e-5,(x['name'],'existing Qt-camera numeric boundary',errors)
  assert len(x['preview'])==len(y['preview']),(x['name'],'preview presence')
  for old,new in zip(x['preview'],y['preview']):
   for a,b in zip(old,new):assert max(abs(v-w)for v,w in zip(a,b))<=2e-5,(x['name'],'preview endpoints')
  rows.append(dict(name=x['name'],point_exact=True,camera_errors=errors,preview=len(y['preview']),undo=0))
 return rows
def ray_compare(source,native):
 a=source['cases'];b=native['cases'];assert len(a)==len(b)==72 and len(set(r['name']for r in a))==72,'ray inventory'
 assert [r['name']for r in a]==[r['name']for r in b],'ray names/order'
 for x,y in zip(a,b):
  for key in ['origin','direction','ground']:assert x[key]==y[key],(x['name'],key,'Qt float mismatch')
 return 72
def main():
 a,b,ra,rb=[json.loads(Path(p).read_text())for p in sys.argv[1:5]];rows=compare(a,b);rays=ray_compare(ra,rb)
 rejected=[]
 for name,mutate in [('missing-case',lambda z:z['cases'].pop()),('wrong-point',lambda z:z['cases'][3]['point'].__setitem__(0,z['cases'][3]['point'][0]+1e-4)),('missing-preview',lambda z:z['cases'][6].__setitem__('preview',[])),('premature-history',lambda z:z['cases'][6].__setitem__('undo',1)),('wrong-handoff',lambda z:z['cases'][0].__setitem__('mode','Position'))]:
  z=copy.deepcopy(b);mutate(z)
  try:compare(a,z)
  except AssertionError:rejected.append(name)
  else:raise AssertionError(f'negative admitted: {name}')
 z=copy.deepcopy(rb);z['cases'][0]['origin'][0]+=1e-4
 try:ray_compare(ra,z)
 except AssertionError:rejected.append('altered-ray')
 else:raise AssertionError('ray negative admitted')
 report=dict(passed=True,cases=rows,exact_ray_cases=rays,rejected_negatives=rejected,scope='actual frozen Qt handlers and controlled egui input; rendered input and image visibility separately required')
 Path(sys.argv[5]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'passed':True,'position_cases':len(rows),'ray_cases':rays,'negatives':len(rejected)}))
if __name__=='__main__':main()
