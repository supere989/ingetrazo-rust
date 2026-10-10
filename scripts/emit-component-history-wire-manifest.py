#!/usr/bin/env python3
"""Readonly manifest from actual captured paths; independent pixel checker judges ink."""
import json,sys
from pathlib import Path
from PySide6.QtGui import QImage
root=Path(sys.argv[1]);phases=[]
initial=json.loads((root/'group-before-state.json').read_text());target=initial['group_ids'][0]
for phase in ['group-before','group-live-preview','group-copy-preview','group-copy-commit','group-array-undo']:
 state=json.loads((root/(phase+'-state.json')).read_text());left,top,w,h=state['viewport_rect']
 def in_frame(segments):
  return len(segments)==24 and all(left+10<=x<left+w-10 and top+10<=y<top+h-10 for segment in segments for x,y in segment)
 wires=[dict(id=wire['group_id'],role='committed',segments=wire['segments']) for wire in state.get('rendered_wire_segments',state['committed_wire_segments']) if in_frame(wire['segments'])]
 assert any(wire['id']==target for wire in wires),'Missing visible selected target observation'
 if phase=='group-copy-preview':wires.append(dict(id='actual-tool-preview',role='preview',segments=state['preview_wire_segments']))
 phases.append(dict(phase=phase,image=phase+'.png',viewport_rect=state['viewport_rect'],wires=wires))
image=QImage(str(root/'group-before.png'));assert not image.isNull()
manifest=dict(protocol='projected-wire-pixels-v1',mode='nested',image_size=[image.width(),image.height()],phases=phases)
(root/'wire-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
