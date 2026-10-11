#!/usr/bin/env python3
import json,sys,os,math
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from formats import igz
from views.viewport import Viewport
from tools.select import SelectTool
p=Path(sys.argv[1]);app=QApplication([]);vp=Viewport();vp.resize(800,600);igz.load_into(vp.scene,p/'input.igz');s=vp.scene
s.groups[0].children[0].mesh.vertex(V(100,100,0));igz.save_scene(s,p/'orphan-input.igz')
vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
point=s.groups[0].xform.map(s.groups[0].children[0].xform.map(V(100,100,0)));pixel=vp._world_to_pixel(point);x,y=pixel
rows=[]
for name,rect,cross in [('orphan-window',[-200,-200,1000,800],False),('orphan-crossing',[x-5,y-5,x+5,y+5],True)]:
 s.selection=set();SelectTool().on_box_select(vp,rect,cross,mode='replace');selected=[]
 for kind,items in [('edge',s.edges),('face',s.faces),('group',s.groups),('guide',s.guides),('dimension',s.dimensions),('text',s.text_labels)]:
  selected.extend([kind,i] for i,e in enumerate(items) if e in s.selection)
 rows.append(dict(name=name,rect=rect,crossing=cross,mode='replace',initial=[],input='orphan-input.igz',orphan=True,selected=sorted(selected)))
assert ['group',0] in rows[0]['selected'];assert ['group',0] not in rows[1]['selected']
a=json.loads((p/'original.json').read_text());a['cases']=[r for r in a['cases'] if not r['name'].startswith('orphan-')]+rows;(p/'original.json').write_text(json.dumps(a,indent=2));print(json.dumps({'new_orphan_controls':2}))
