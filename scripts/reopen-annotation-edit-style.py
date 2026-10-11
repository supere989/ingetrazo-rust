#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from views.viewport import Viewport
from formats import igz
app=QApplication([]);p=Path(sys.argv[1]);cases=[]
for kind in ['label','dimension']:
 for context in ['root','nested']:
  name=kind+'-'+context;row={'case':name}
  for role,prefix in [('original',''),('native','native-')]:
   v=Viewport();igz.load_into(v.scene,p/(prefix+name+'.igz'));s=v.scene
   row[role]={'labels':[t.text for t in s.text_labels],'dimensions':[d.text for d in s.dimensions],'style':s.dimension_style,'groups':[(g.name,len(g.children),len(g.mesh.vertices))for g in s.groups],'vertices':[list(x.position.toTuple())for x in s.mesh.vertices]}
  cases.append(row)
(p/'reopen.json').write_text(json.dumps({'cases':cases},indent=2))
