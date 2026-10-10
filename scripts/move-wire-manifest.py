#!/usr/bin/env python3
import argparse,json
from pathlib import Path
from PySide6.QtGui import QImage
p=argparse.ArgumentParser();p.add_argument('capture_dir');p.add_argument('mode',choices=['ordinary','linked','nested']);a=p.parse_args();root=Path(a.capture_dir);phases=[]
for name in ['group-before','group-live-preview','group-copy-preview','group-copy-commit','group-array-undo']:
 s=json.loads((root/(name+'-state.json')).read_text());assert s['capture_phase']==name
 im=QImage(str(root/(name+'.png')));assert not im.isNull()
 l,t,r,b=s['viewport_rect'];wires=[];offscreen=[]
 for row in s.get('rendered_wire_segments',s['committed_wire_segments']):
  inside=all(l+10<=x<r-10 and t+10<=y<b-10 for seg in row['segments']for x,y in seg)
  if inside:wires.append({'id':row['group_id'],'role':'committed','segments':row['segments']})
  else:offscreen.append(row['group_id'])
 if name=='group-copy-preview':wires.append({'id':'actual-preview','role':'preview','segments':s['preview_wire_segments']})
 phases.append({'phase':name,'image':name+'.png','viewport_rect':[l,t,r-l,b-t],'wires':wires,'offscreen_committed_not_claimed':offscreen})
manifest={'protocol':'projected-wire-pixels-v1','mode':a.mode,'image_size':[im.width(),im.height()],'phases':phases,'scope':'Readonly actual transformed Mesh and tool preview projections, in-frame target visibility only; offscreen siblings not claimed'}
(root/'wire-pixel-manifest.json').write_text(json.dumps(manifest,indent=2))
