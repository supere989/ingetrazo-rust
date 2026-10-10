#!/usr/bin/env python3
"""Require observed target ink before/after real context-entry pointer gestures."""
import importlib.util,json,math,sys
from pathlib import Path
from PySide6.QtGui import QImage
spec=importlib.util.spec_from_file_location('wire_pixels',Path(__file__).with_name('check-move-wire-pixels.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
def inspect_one(root,phase):
 root=Path(root);state=json.loads((root/(phase+'-state.json')).read_text());point=state['entry_pixel']
 wire=next(w for w in state.get('rendered_wire_segments',state['committed_wire_segments']) if len(w['segments'])==24 and any(math.dist(p,point)<1e-6 for segment in w['segments']for p in segment))
 left,top,width,height=state['viewport_rect']
 assert all(left+10<=x<left+width-10 and top+10<=y<top+height-10 for segment in wire['segments']for x,y in segment),'Entry target outside actual world viewport'
 image=QImage(str(root/(phase+'.png')));assert not image.isNull()
 value=module.wire_coverage(image,wire['segments'])
 result=dict(phase=phase,group_id=wire['group_id'],entry_pixel=point,passed=value['fraction']>=.75 and min(value['quarters'])>=.5,**value)
 (root/(phase+'-pixels.json')).write_text(json.dumps(result,indent=2)+'\n')
 assert result['passed'],'Entry target has insufficient distributed visible ink'
 return result
if __name__=='__main__':print(json.dumps(inspect_one(sys.argv[1],sys.argv[2])))
