#!/usr/bin/env python3
"""Whole authored loader/render boundary, default failure retained until fixed."""
import json,sys,copy
from pathlib import Path
from PySide6.QtGui import QImage
EXPECTED=['authored-legacy-absent','authored-legacy-null','authored-legacy-null-nullxform','authored-legacy-null-component-true','authored-proto-zero-true','authored-proto-zero-false','authored-nested-numeric-adopt','authored-nested-null-adopt']
def validate_groups(source,native):
 assert [r['case']for r in source]==EXPECTED==[r['case']for r in native]
 for s,n in zip(source,native):
  assert len(s['groups'])==len(n['groups'])
  for a,b in zip(s['groups'],n['groups']):
   for key in ['path','uid','is_component','ext']:assert a[key]==b[key],(s['case'],key,a[key],b[key])
   assert a['component']==b['component_flag'],(s['case'],'authored component flag')
   assert len(a['world_points'])==len(b['world_points'])
   assert all(abs(x-y)<1e-6 for pa,pb in zip(a['world_points'],b['world_points'])for x,y in zip(pa,pb)),(s['case'],'world geometry')
   if a['xform']is not None:assert a['xform']==b['transform'],(s['case'],'columnmajor transform')
def main():
 source_root,native_root,window_root,observed_root=map(Path,sys.argv[1:5])
 s=json.loads((source_root/'original-alpha-results.json').read_text())['cases'];n=json.loads((native_root/'native-alpha-results.json').read_text())['cases'];w=json.loads((window_root/'native-ui-results.json').read_text())['cases'];g=json.loads((observed_root/'native-group-observations.json').read_text())['cases']
 validate_groups(s,g)
 assert [r['case']for r in s]==EXPECTED==[r['case']for r in n]==[r['case']for r in w]
 failures=[]
 for a,b,c in zip(s,n,w):
  assert 'NVIDIA GeForce RTX 5090'in a['gl_info']['renderer']and not a['gl_info']['software']
  for root,row in [(source_root,a),(native_root,b),(window_root,c)]:
   im=QImage(str(root/(row['case']+'.png')));assert not im.isNull()and(im.width(),im.height())==(800,600)
   p=im.pixelColor(400,300);assert[p.red(),p.green(),p.blue(),p.alpha()]==row['center_rgba']
  if a['center_rgba']!=b['center_rgba']or a['center_rgba']!=c['center_rgba']or a['ordinary_faces_count']!=b['consolidated_opaque_count']:
   failures.append({'case':a['case'],'source':a['center_rgba'],'headless':b['center_rgba'],'window':c['center_rgba'],'source_ordinary_count':a['ordinary_faces_count'],'native_ordinary_count':b['consolidated_opaque_count']})
 rejected=0
 for mode in ['missing','world','component']:
  bad=copy.deepcopy(g)
  if mode=='missing':bad.pop()
  elif mode=='world':bad[6]['groups'][1]['world_points'][0][0]+=1
  else:bad[4]['groups'][0]['is_component']=False
  try:validate_groups(s,bad)
  except AssertionError:rejected+=1
  else:raise AssertionError('Negative accepted '+mode)
 source_roundtrip=[{'case':r['case'],'path':a['path'],'before':a['component'],'after':b['component']}for r in s for a,b in zip(r['groups'],r['original_roundtrip_groups'])if a['component']!=b['component']]
 result={'passed':not failures,'authored_cases':8,'native_loader_save_reopen_semantics':True,'negative_controls':rejected,'literal_render_failures':failures,'source_roundtrip_classic_flag_losses':source_roundtrip,'scope':'SameGPU authored original/native load/render, geometry/component/UID/metadata. No editing/general group claim.'}
 print(json.dumps(result,indent=2))
 if '--retain-open'in sys.argv:assert len(failures)==1 and failures[0]['case']=='authored-nested-null-adopt'
 else:assert not failures,failures
if __name__=='__main__':main()
