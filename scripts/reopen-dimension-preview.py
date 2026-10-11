#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
app=QApplication([]);p=Path(sys.argv[1]);rows=[]
for case in ['front-z','side-x','aligned','axial-x','axial-y','nested']:
 observations=[]
 for suffix in ['original','native']:
  s=Scene();igz.load_into(s,p/(case+'-'+suffix+'.igz'));observations.append(dict(dimensions=[dict(a=[d.a.x(),d.a.y(),d.a.z()],b=[d.b.x(),d.b.y(),d.b.z()],offset=[d.offset.x(),d.offset.y(),d.offset.z()],axis=d.axis,text=getattr(d,'text_override',None),value=d.value(),label=d.label())for d in s.dimensions],units=s.units,groups=[g.uid for g in s.groups]))
 assert observations[0]==observations[1],(case,observations)
 rows.append(dict(case=case,original_reader=observations[1]))
(p/'preview-reopens.json').write_text(json.dumps(dict(cases=rows),indent=2));print(json.dumps(dict(original_reopens=6,exact=True)))
