#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from formats import igz
app=QApplication([]);p=Path(sys.argv[1]);rows=[]
def point(p):return list(p.toTuple())
def observe(s):return [dict(a=point(d.a),b=point(d.b),offset=point(d.offset),axis=d.axis,anchored=[d.anchor_a is not None,d.anchor_b is not None])for d in s.dimensions]
for case in ['near','outside','nested-shared','billboard','orphan']:
 sources=[]
 for suffix in ['original','native']:
  s=Scene();igz.load_into(s,p/(case+'-'+suffix+'.igz'));before=observe(s)
  shared=case!='nested-shared' or s.groups[0].children[0].mesh is s.groups[0].children[1].mesh
  assert shared,'shared placement prototype split on reopen'
  d=s.dimensions[0]
  if d.anchor_b is not None:
   h=d.anchor_b;h.mesh.place_vertex(h.vertex,h.vertex.position+V(0,0,1))
  after=observe(s);sources.append(dict(before=before,after=after,shared=shared))
 assert sources[0]==sources[1],(case,sources)
 rows.append(dict(case=case,original_reader=sources[1]))
(p/'admission-reopens.json').write_text(json.dumps(dict(cases=rows),indent=2));print(json.dumps(dict(original_reopens=5,live_follow=2,exact=True)))
