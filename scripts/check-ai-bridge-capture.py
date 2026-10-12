#!/usr/bin/env python3
"""Strict finite screenshot pixels and declared model state; no tolerance or renderer oracle."""
import argparse,hashlib,json,math
from pathlib import Path
from PySide6.QtGui import QImage
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
ROSTER=[('default-perspective',1),('clamped-and-portrait',3),('orthographic-live-depth',2),('committed-selected-overlays',1),('active-edit-context',1),('invalid-dimensions',2)]
CONTROLS={'actual_socket','actual_host_renderer','requested_rerender','readonly_model_history_camera','nonuniform_actual_png','worker_joined'}
def exact(a,b,path='$'):
 if type(a)is not type(b):raise AssertionError(f'{path}: types differ')
 if isinstance(a,float):
  assert math.isfinite(a)and math.isfinite(b),path+' nonfinite'
  assert a==b and(a!=0 or math.copysign(1,a)==math.copysign(1,b)),path+' float differs'
 elif isinstance(a,dict):
  assert a.keys()==b.keys(),path+' keys differ'
  for k in a:exact(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),path+' length differs'
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,f'{path}[{i}]')
 else:assert a==b,path+' value differs'
def owned(root,name):
 p=Path(name);assert not p.is_absolute()and '..'not in p.parts and p.parts,'unsafe artifact path'
 target=root/p;assert target.is_file()and not target.is_symlink(),'missing artifact';return target

def image(root,row):
 v=row['image'];assert set(v)=={'file','rgba','size','encoded_sha256','rgba_sha256'}
 encoded=owned(root,v['file']).read_bytes();raw=owned(root,v['rgba']).read_bytes()
 assert hashlib.sha256(encoded).hexdigest()==v['encoded_sha256']and hashlib.sha256(raw).hexdigest()==v['rgba_sha256'],'image byte binding'
 size=v['size'];assert len(size)==2 and all(type(x)is int and 64<=x<=4096 for x in size),'image dimensions'
 assert len(raw)==size[0]*size[1]*4,'RGBA extent'
 q=QImage(str(owned(root,v['file'])));assert not q.isNull()and[q.width(),q.height()]==size,'actual PNG decode'
 q=q.convertToFormat(QImage.Format_RGBA8888);data=bytes(q.constBits());decoded=b''.join(data[y*q.bytesPerLine():y*q.bytesPerLine()+size[0]*4]for y in range(size[1]));assert decoded==raw,'PNG/RGBA disagreement'
 assert any(raw[i:i+4]!=raw[:4]for i in range(0,len(raw),4)),'uniform image'
 return raw

def state(value,case):
 assert set(value)=={'selection','scene','overlays','camera','undo','redo','edit_context','current_file','dirty','scene_version'},'state inventory'
 for k in ['undo','redo','edit_context','scene_version']:assert type(value[k])is int and value[k]>=0,k
 assert type(value['dirty'])is bool
 assert type(value['current_file'])is str and Path(value['current_file']).name==case['initial_file'],'declared current file'
 assert set(value['scene'])=={'active','loose','groups','layers','units','datum'},'scene inventory'
 assert set(value['overlays'])=={'dimensions','text_labels','guides','section_planes','geo_paths','geo_points'},'overlay inventory'
 assert set(value['camera'])=={'target','distance','yaw','pitch','perspective','fov','aspect','up','znear','zfar','two_point'},'camera inventory'
 # Epoch and capture/replay directory identity are per-runtime diagnostics. All other fields stay strict.
 return {**value,'current_file':case['initial_file'],'scene_version':None}

def compare(source,native,sroot,nroot):
 assert source['source_pin']==native['source_pin']==PIN
 assert [(c['name'],len(c['requests']))for c in source['cases']]==ROSTER
 exact(source['cases'],native['cases'],'$.cases');assert source['callback_errors']==[]
 assert len(source['states'])==len(native['states'])==10 and len(source['render_calls'])==8
 assert set(native['controls'])==CONTROLS and all(type(v)is bool and v for v in native['controls'].values()),'native controls'
 for r in source['render_calls']:
  assert all(type(r[k])is bool and r[k]for k in ['live_target_restored','live_depth_retained','ordinary_world_rgba_unchanged','scene_readonly']),'source invariance guard'
 cases={c['name']:c for c in source['cases']};rows=[];images=0
 for a,b in zip(source['states'],native['states']):
  try:
   exact({k:a[k]for k in ['case','index','request']},{k:b[k]for k in ['case','index','request']});case=cases[a['case']]
   exact(a['before'],a['after'],'$.source.readonly');exact(b['before'],b['after'],'$.native.readonly')
   exact(state(a['before'],case),state(b['before'],case),'$.state')
   if a['case']=='invalid-dimensions':
    assert a['reply']['ok']is False and b['reply']['ok']is False
    exact(a['reply'],b['reply'],'$.literal_refusal');assert 'image'not in a and'image'not in b
   else:
    assert a['reply']['ok']is True and b['reply']['ok']is True
    exact({k:a['reply'][k]for k in ['id','ok']},{k:b['reply'][k]for k in ['id','ok']})
    for r in [a,b]:
     assert set(r['reply']['result'])=={'path','width','height'}
     assert Path(r['reply']['result']['path']).name=='viewport.png'and Path(r['reply']['result']['path']).parent.name=='ingetrazo-ai','actual response path scope'
     exact(r['image']['size'],[r['reply']['result']['width'],r['reply']['result']['height']])
    ar=image(sroot,a);br=image(nroot,b);exact(a['image']['size'],b['image']['size']);assert ar==br,'strict decoded RGBA differs';images+=1
   rows.append({'case':a['case'],'index':a['index'],'ok':True})
  except(AssertionError,KeyError,TypeError,ValueError)as e:rows.append({'case':a.get('case'),'index':a.get('index'),'ok':False,'error':str(e)})
 return {'ok':all(r['ok']for r in rows),'cases':6,'replies':10,'exact_rows':sum(r['ok']for r in rows),'strict_images':images,'rows':rows,'negative_controls_accepted':0,'scope':'Decoded viewport RGBA and strict declared state/requests/literal refusals. Encoded PNG compression and runtime directories/Scene epoch are diagnostics; each runtime readonly state remains exact. No OS chrome, arbitrary interactive preview, GPU platform parity or performance claim.'}

def main():
 p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args()
 try:result=compare(json.loads((a.source/'original-ai-bridge-capture.json').read_text()),json.loads((a.native/'native-ai-bridge-capture.json').read_text()),a.source,a.native)
 except(Exception)as e:result={'ok':False,'error':str(e),'negative_controls_accepted':0}
 a.comparison.parent.mkdir(parents=True,exist_ok=True);a.comparison.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n');print(json.dumps(result));raise SystemExit(0 if result['ok']else 1)
if __name__=='__main__':main()
