#!/usr/bin/env python3
"""Strict same-device 58-case source/headless/window paint acceptance."""
import json,sys
from pathlib import Path
from PySide6.QtGui import QImage
BASE=['red-half-white','red-half-black','opacity-bucket-order','opaque-front','opaque-back','equal-opacity-forward','equal-opacity-reversed','opacity-995','opacity-zero','section-clipped']
ROUND=['rgb-tie-even','rgb-tie-odd','opacity-round-5005','opacity-round-1255','opacity-threshold-f64']
BACK=[f'back-{a}-{b}-{d}'for a,b in [(0.5,0.3),(1.,0.3),(0.5,1.),(1.,1.)]for d in ['front','reversed']]+['back-gate-activated-front','back-gate-activated-reversed','back-order-translucent-opaque','back-order-opaque-translucent']
GROUP=['back-gate-instanced-only-front','back-gate-instanced-only-reversed','group-rotated-prototype','group-nested-rotated-prototype','group-classic-baked-rotation','paint-default-front','paint-default-reversed','paint-face-override','paint-explicit-back','paint-child-own','paint-top-owner-not-intermediate']
STYLE=[f"style-{mode}-{kind}"for mode in ["wireframe","hidden_line","monochrome","xray","shaded"]for kind in ["front","reverse","translucent","explicit-back"]]
EXPECTED=BASE+ROUND+BACK+GROUP+STYLE
assert len(EXPECTED)==58 and len(set(EXPECTED))==58
def validate(source,native,window):
 for rows in [source,native,window]:assert [r['case']for r in rows]==EXPECTED,'Missing, duplicated, extra or reordered case'
 for s,n,w in zip(source,native,window):
  assert 'NVIDIA GeForce RTX 5090'in s['gl_info']['renderer'] and not s['gl_info']['software']
  assert s['center_rgba']==n['center_rgba']==w['center_rgba'],(s['case'],s['center_rgba'],n['center_rgba'],w['center_rgba'])
  assert n['geometry_uploads'] in ((0,1) if s['case'].startswith('style-') else (1,)) and n['surface_bytes']%24==0
  assert 'additional_position_bytes: 0'in n['contour_metrics'],'Transparent vertex duplication'
  assert w['passed'] is True
  if s['case'].startswith('style-'):
   assert s['style_mode']==s['case'].split('-')[1]
   assert s['style_front_color']==[.2,.4,.6] and s['style_back_color']==[.8,.3,.1]

def main():
 dirs=list(map(Path,sys.argv[1:4]));assert len(dirs)==3
 rows=[json.loads((p/f).read_text())['cases']for p,f in zip(dirs,['original-alpha-results.json','native-alpha-results.json','native-ui-results.json'])]
 validate(*rows)
 for p,data in zip(dirs,rows):
  for r in data:
   im=QImage(str(p/(r['case']+'.png')));assert not im.isNull() and (im.width(),im.height())==(800,600)
   c=im.pixelColor(400,300);assert [c.red(),c.green(),c.blue(),c.alpha()]==r['center_rgba']
 import copy
 rejected=0
 for mode in ['missing','duplicate','rgba','duplicate_xyz','missing_style_metadata']:
  bad=copy.deepcopy(rows)
  if mode=='missing':bad[1].pop()
  elif mode=='duplicate':bad[1][-1]=bad[1][0]
  elif mode=='rgba':bad[1][0]['center_rgba'][0]^=1
  elif mode=='duplicate_xyz':bad[1][0]['contour_metrics']='additional_position_bytes: 24'
  else:bad[0][-1].pop('style_mode')
  try:validate(*bad)
  except (AssertionError,KeyError):rejected+=1
  else:raise AssertionError('Negative control accepted: '+mode)
 baseline=json.loads(Path(sys.argv[4]).read_text())['cases'];assert [r['case']for r in baseline]==EXPECTED
 assert all(a['center_rgba']==b['center_rgba']for a,b in zip(rows[0][:38],baseline[:38]))
 old_failures=sum(a['center_rgba']!=b['center_rgba']for a,b in zip(rows[0][38:],baseline[38:]));assert old_failures==15
 print(json.dumps({'retained_old_style_failures':old_failures,'passed':True,'mandatory_cases':58,'negative_controls':rejected,'rgba_tolerance':0,'scope':'Same NVIDIA adapter, source OpenGL/headless Vulkan/actual owned-window Vulkan. No general texture/style/Metal acceptance.'},indent=2))
if __name__=='__main__':main()
