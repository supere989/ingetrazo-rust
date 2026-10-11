#!/usr/bin/env python3
"""Exact native placement interiors/backgrounds; not raster-edge/UI acceptance."""
import json,pathlib,sys
NAMES=("shared-prototype-reflected","warm-left","warm-center","warm-right","winding-disabled-negative","old-retained-green","new-revision-red","binding-only-reuse","refusal-preserved")
def validate(pixels,name,positive_override=False):
 assert len(pixels)==64*64*4
 left={"warm-left":-.5,"warm-right":-.3}.get(name,-.4)
 single=name in ("binding-only-reuse","refusal-preserved")
 red=name in ("new-revision-red","binding-only-reuse","refusal-preserved")
 copies=[(left,(255,0,0,255) if red else (0,255,0,255))]
 if not single:copies.append((.4,(255,255,0,255) if name=="winding-disabled-negative" and not positive_override else ((255,0,0,255) if red else (0,255,0,255))))
 inside=outside=0
 for y in range(64):
  for x in range(64):
   px=2*(x+.5)/64-1;py=1-2*(y+.5)/64;half=(.25-py)/2
   actual=tuple(pixels[(y*64+x)*4:(y*64+x)*4+4])
   expected=next((color for offset,color in copies if -.23<py<.23 and abs(px-offset)<half-.02),None)
   if expected is not None:assert actual==expected,(name,x,y,actual,expected);inside+=1
   elif all(py<-.27 or py>.27 or abs(px-offset)>half+.02 for offset,_ in copies):assert actual==(0,0,255,255),(name,x,y,actual,'background');outside+=1
 assert inside>70 and outside>3000,(name,inside,outside)
 return {'interior_pixels':inside,'background_pixels':outside}
def main(root):
 root=pathlib.Path(root);report=json.loads((root/'report.json').read_text());assert report['passed'] is True
 assert report['geometry_uploads_before_warm']==report['geometry_uploads_after_warm']==1
 assert report['targets_before_warm']==report['targets_after_warm']==1
 assert report['pose_bytes_before_warm']==report['pose_bytes_after_warm']>0
 assert report['cache_bytes_before_warm']==report['cache_bytes_after_warm']>0
 assert report['final_uploads']==2 and report['ledger_committed']==report['ledger_reserved']==0
 assert report['wrong_owner_refused'] is True and report['invalid_pose_refused'] is True
 assert {p.stem for p in root.glob('*.rgba')}==set(NAMES)
 frames={n:(root/(n+'.rgba')).read_bytes() for n in NAMES};results={n:validate(p,n) for n,p in frames.items()}
 for label,pixels,name,override in [('blank',bytes([0,0,255,255])*4096,'shared-prototype-reflected',False),('winding-disabled-as-positive',frames['winding-disabled-negative'],'winding-disabled-negative',True),('new-as-old',frames['new-revision-red'],'old-retained-green',False)]:
  try:validate(pixels,name,override)
  except AssertionError:pass
  else:raise AssertionError(label)
 print(json.dumps({'passed':True,'frames':results,'negatives':'blank/winding-disabled-as-positive/new-as-old rejected','scope':'actual cached local prototypes, per-binding matrices/winding, warm retained capacities and old-frame/refusal ownership; interior/background only; instance batching/fullScene/UI/presentation not claimed'},indent=2))
if __name__=='__main__':main(sys.argv[1])
