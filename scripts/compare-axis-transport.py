#!/usr/bin/env python3
"""Compare retained actual axes transport. Diagnostic, never pixel acceptance."""
import argparse,json,struct,hashlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args()
s=json.loads((a.source/'source-axis-transport.json').read_text());n=json.loads((a.native/'native-axis-transport.json').read_text())
sb=(a.source/'source-axis-vbo.bin').read_bytes();nb=(a.native/'native-axis-vbo.bin').read_bytes()
assert len(sb)==3126*12 and len(nb)==3126*24
assert s["vbo"]["size"]==len(sb) and s["vbo"]["sha256"]==hashlib.sha256(sb).hexdigest()
assert n["vertex_count"]==3126 and n["stride"]==24 and n["position_offset"]==0 and n["color_offset"]==12
rows=s['draws'];assert len(rows)==3 and n['readonly'] is True and n['before']==n['after']
position_differences=[];color_differences=[]
for i in range(3126):
 src=struct.unpack_from('<3I',sb,i*12);dst=struct.unpack_from('<3I',nb,i*24)
 if src!=dst:position_differences.append({'vertex':i,'source_bits':src,'native_bits':dst})
 color=struct.unpack_from('<3I',nb,i*24+12);row=next(x for x in rows if x['first']<=i<x['first']+x['count'])
 if tuple(row['color']['bits'][:3])!=color:color_differences.append({'vertex':i,'source_bits':row['color']['bits'][:3],'native_bits':color})
report={'diagnostic_only':True,'position_differences':len(position_differences),'position_examples':position_differences[:32],'color_differences':len(color_differences),'color_examples':color_differences[:8],'source_mvp_bits':rows[0]['mvp']['bits'],'native_mvp_bits':n['mvp_bits'],'matrix_bits_equal':rows[0]['mvp']['bits']==n['mvp_bits'],'source_viewport':rows[0]['viewport'],'native_extent':n['extent'],'source_clip_enable':[x['clip_enabled'] for x in rows],'native_draw_flags':n['draw_flags'],'source_raster_states':[x['raster_state'] for x in rows],'native_pipeline_recipe':n['pipeline_recipe']}
(a.native/'axis-transport-comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
