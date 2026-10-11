#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from views.viewport import Viewport
from formats import igz
app=QApplication([]);p=Path(sys.argv[1]);rows=[]
names=[c+'-'+k for c in ['root','nested'] for k in ['edge','face','coordinates']]+['closed-face']
v=lambda p:list(p.toTuple())
for name in names:
 row={'case':name}
 for role,suffix in [('original',''),('native','-native')]:
  viewport=Viewport();igz.load_into(viewport.scene,p/(name+suffix+'.igz'));s=viewport.scene
  row[role]={'labels':[dict(anchor=v(t.anchor),offset=v(t.offset),text=t.text,layer=getattr(t,'layer',None))for t in s.text_labels],'unit':s.units['length'],'precision':s.units['precision'],'groups':[(g.uid,g.name,len(g.children),len(g.mesh.vertices))for g in s.groups],'vertices':[v(t.position)for t in s.mesh.vertices]}
 rows.append(row)
(p/'reopen.json').write_text(json.dumps(dict(cases=rows),indent=2))
