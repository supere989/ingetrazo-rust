#!/usr/bin/env python3
"""Compare retained actual axes transport. Diagnostic, never pixel acceptance."""
import argparse,json,struct,hashlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args()
s=json.loads((a.source/'source-axis-transport.json').read_text());n=json.loads((a.native/'native-axis-transport.json').read_text())
nt=n.get('transport',n)
sb=(a.source/'source-axis-vbo.bin').read_bytes();nb=(a.native/'native-axis-vbo.bin').read_bytes()
assert len(sb)==3126*12 and len(nb)==3126*24
assert s["vbo"]["size"]==len(sb) and s["vbo"]["sha256"]==hashlib.sha256(sb).hexdigest()
assert nt["vertex_count"]==3126 and nt["stride"]==24 and nt["position_offset"]==0 and nt["color_offset"]==12
rows=s['draws'];assert len(rows)==3 and n['readonly'] is True and n['before']==n['after']
position_differences=[];color_differences=[]
for i in range(3126):
 src=struct.unpack_from('<3I',sb,i*12);dst=struct.unpack_from('<3I',nb,i*24)
 if src!=dst:position_differences.append({'vertex':i,'source_bits':src,'native_bits':dst})
 color=struct.unpack_from('<3I',nb,i*24+12);row=next(x for x in rows if x['first']<=i<x['first']+x['count'])
 if tuple(row['color']['bits'][:3])!=color:color_differences.append({'vertex':i,'source_bits':row['color']['bits'][:3],'native_bits':color})
report={'diagnostic_only':True,'position_differences':len(position_differences),'position_examples':position_differences[:32],'color_differences':len(color_differences),'color_examples':color_differences[:8],'source_mvp_bits':rows[0]['mvp']['bits'],'native_mvp_bits':nt['mvp_bits'],'matrix_bits_equal':rows[0]['mvp']['bits']==nt['mvp_bits'],'source_viewport':rows[0]['viewport'],'native_extent':nt['extent'],'source_clip_enable':[x['clip_enabled'] for x in rows],'native_draw_flags':nt['draw_flags'],'source_raster_states':[x['raster_state'] for x in rows],'native_pipeline_recipe':nt['pipeline_recipe'],'source_attachments':[x['attachments'] for x in rows],'native_attachment_recipe':nt['attachment_recipe']}
if s.get('sample_counts'):
 assert s['positive_control_passed'] is True and n['positive_control_passed'] is True
 assert s['requested_gpu_draw_only'] is True and s['live_fbo_restored'] is True and s['live_depth_restored'] is True
 assert n['requested_extent']==[1024,768] and n['precise_feature_admitted'] is True and n['live_target_restored'] is True and n['color_writes'] is False and n['depth_writes'] is False
 def indexed(items):
  assert len(items)==48
  result={}
  for row in items:
   key=(*row['site_top_origin'],row['axis_first'],row['sample'])
   assert key not in result and row['attachment_pixel']==[key[0],767-key[1]]
   counts=row['counts'];assert set(counts)=={'depth_enabled','depth_disabled'} and all(type(v)is int and v>=0 for v in counts.values())
   assert counts['depth_enabled']<=counts['depth_disabled']
   result[key]=counts
  return result
 si=indexed(s['sample_counts']);ni=indexed(n['rows']);assert set(si)==set(ni)
 report['sample_count_comparison']=[{'key':key,'source':si[key],'native':ni[key]} for key in si if si[key]!=ni[key]]
 report['sample_count_scope']='Precise coverage/depth-passing counts; not raw attachment colors/depth or pixel acceptance'
(a.native/'axis-transport-comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
