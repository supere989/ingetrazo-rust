#!/usr/bin/env python3
"""Exact source state/path and actual retained item-paint comparison; no GPU framebuffer claim."""
import argparse,copy,json,math
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage
NAMES=['fresh-chain','fresh-baseline','seeded-chain','seeded-baseline','run-lifecycle']
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def equal(a,b,path='$'):
 if type(a)in(int,float)and type(b)in(int,float):
  assert math.isfinite(a)and math.isfinite(b),path
  assert a==b and(a!=0 or math.copysign(1,a)==math.copysign(1,b)),(path,a,b)
  return
 assert type(a)is type(b),(path,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys',list(a),list(b))
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+f'[{i}]')
 else:assert a==b,(path,a,b)
def verify(s,n,readers):
 assert s['source_pin']==PIN and n['source_pin']==PIN and n['failures']==[]
 assert [c['name']for c in s['cases']]==NAMES==[c['name']for c in n['cases']]
 expected=[(c['name'],a['phase'])for c in s['cases']for a in c['actions']];assert len(expected)==106
 assert [(r['case'],r['phase'])for r in s['states']]==expected==[(r['case'],r['phase'])for r in n['states']]
 assert n['actual_app_paint']is True
 assert [c['case']for c in n['controls']]==NAMES
 for c in n['controls']:
  for k in ['actual_pointer_ingress','hover_readonly','idle_reused']:assert c[k]is True,(c['case'],k)
 for i,(a,b)in enumerate(zip(s['states'],n['states'])):
  for k in ['undo','redo','sheet_index']:assert type(a['state'][k])is int and type(b['state'][k])is int and a['state'][k]>=0 and b['state'][k]>=0
  equal(a['state'],b['state'],f'states[{i}].state');assert a['observer_readonly']is True and b['observer_readonly']is True
  assert type(b['native_scene_version'])is int and b['native_scene_version']>=0
  equal(a['preview'],b['preview'],f'states[{i}].preview')
  for k in ['density','dpr','clip_mm','background','page_to_device','nonempty']:equal(a['paint'][k],b['paint'][k],f'states[{i}].paint.'+k)
  assert type(b['paint']['submitted_callbacks'])is int and b['paint']['submitted_callbacks']>=0
  if a['paint']['nonempty']:assert b['paint']['submitted_callbacks']>0,(a['case'],a['phase'],'not actual App paint')
 assert [r['case']for r in readers]==NAMES
 for c,r in zip(s['cases'],readers):
  last=next(x for x in reversed(s['states'])if x['case']==c['name']);equal(last['state']['compositions'],r['compositions'],'reader.'+c['name'])
def pixels(path):
 image=QImage(str(path));assert not image.isNull(),str(path);image=image.convertToFormat(QImage.Format_RGB888);assert(image.width(),image.height())==(594,420)
 return np.frombuffer(image.constBits(),np.uint8).reshape(420,image.bytesPerLine())[:,:594*3].reshape(420,594,3).copy()
def pixel_difference(src,dst):
 delta=np.abs(src.astype(np.int16)-dst.astype(np.int16));return {'changed_pixels':int(np.count_nonzero(np.any(delta,axis=2))),'max_channel':int(delta.max())}
def require_same_pixels(src,dst):
 result=pixel_difference(src,dst);assert result['changed_pixels']==0,('Strict primitive pixel mismatch',result);return result
def main():
 p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args();s=json.loads((a.source/'original-run-previews.json').read_text());n=json.loads((a.native/'native-run-previews.json').read_text());readers=json.loads((a.native/'native-reopens.json').read_text());report={'ok':False,'semantic_ok':False,'strict_pixels_ok':False,'state_mismatches':[],'pixels':[]}
 try:
  for i,(x,y)in enumerate(zip(s['states'],n['states'])):
   try:equal(x['state'],y['state']);equal(x['preview']['elements'],y['preview']['elements'])
   except AssertionError as e:report['state_mismatches'].append({'case':x['case'],'phase':x['phase'],'error':str(e)})
  for x,y in zip(s['states'],n['states']):
   src=pixels(a.source/x['paint']['file']);dst=pixels(a.native/y['paint']['file']);row=dict(case=x['case'],phase=x['phase'],**pixel_difference(src,dst));report['pixels'].append(row)
  verify(s,n,readers)
  report['strict_pixels_ok']=all(r['changed_pixels']==0 for r in report['pixels'])
  mutations=[('missing-state',lambda d:d['states'].pop()),('extra-state',lambda d:d['states'].append(copy.deepcopy(d['states'][-1]))),('phase-order',lambda d:d['states'].reverse()),('history-bool',lambda d:d['states'][0]['state'].__setitem__('undo',False)),('history',lambda d:d['states'][0]['state'].__setitem__('undo',1)),('pending-axis',lambda d:d['states'][0]['state']['pending'].__setitem__('run_axis','h')),('path-point',lambda d:d['states'][4]['preview']['elements'][1].__setitem__('x',999.)),('preview-presence',lambda d:d['states'][0]['preview'].__setitem__('exists',True)),('no-actual-paint',lambda d:d.__setitem__('actual_app_paint',False)),('no-submission',lambda d:d['states'][4]['paint'].__setitem__('submitted_callbacks',0)),('selection',lambda d:d['states'][0]['state']['selection'].append(0)),('observer',lambda d:d['states'][0].__setitem__('observer_readonly',False))]
  for flag in ['actual_pointer_ingress','hover_readonly','idle_reused']:mutations.append((flag,lambda d,k=flag:d['controls'][0].__setitem__(k,False)))
  rejected=[]
  for name,f in mutations:
   bad=copy.deepcopy(n);f(bad)
   try:verify(s,bad,readers)
   except AssertionError:rejected.append(name)
   else:raise AssertionError('Negative did not reject: '+name)
  bad=copy.deepcopy(readers);bad.pop()
  try:verify(s,n,bad)
  except AssertionError:rejected.append('missing-reader')
  else:raise AssertionError('Reader negative did not reject')
  bad=copy.deepcopy(s);bad['source_pin']='wrong'
  try:verify(bad,n,readers)
  except AssertionError:rejected.append('wrong-source-pin')
  else:raise AssertionError('Pin negative did not reject')
  changed=pixels(a.source/s['states'][4]['paint']['file']);changed[0,0,0]^=1
  try:require_same_pixels(changed,pixels(a.source/s['states'][4]['paint']['file']))
  except AssertionError:rejected.append('one-pixel')
  else:raise AssertionError('Pixel negative did not reject')
  report.update(semantic_ok=True,cases=5,states=106,original_reader_pairs=5,rejected_controls=rejected,scope='Exact actual App state/path/style/pose and saved-file readers are separate from strict source item-paint versus submitted CPU coverage pixels. Viewport grabs retained separately; no GPU/full UI pixel claim.')
  report['pixel_exact_states']=sum(r['changed_pixels']==0 for r in report['pixels']);report['pixel_residual_states']=sum(r['changed_pixels']>0 for r in report['pixels'])
  assert report['strict_pixels_ok'],'Strict primitive pixel mismatch; semantic acceptance is separately reported'
  report['ok']=True
 except Exception as e:report['error']=repr(e);a.comparison.write_text(json.dumps(report,indent=2)+'\n');raise
 a.comparison.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
