#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from core.scene import Scene
from formats import igz
out=Path(sys.argv[1]);rows=[]
def state(path):
 s=Scene();igz.load_into(s,path)
 def mesh(m):return dict(vertices=[list(v.position.toTuple()) for v in m.vertices],edges=[sorted([list(e.a.toTuple()),list(e.b.toTuple())]) for e in m.edges],faces=[dict(outer=[list(v.toTuple()) for v in f.vertices],holes=[[list(v.toTuple())for v in ring]for ring in f.holes],attrs=f.attrs)for f in m.faces])
 def group(g):return dict(name=g.name,mesh=mesh(g.mesh),children=[group(c)for c in g.children])
 return dict(points=[p.to_dict() for p in s.geo_points],datum=s.georef.to_dict() if s.georef else None,units=s.units,mesh=mesh(s.mesh),groups=[group(g)for g in s.groups])
for n in ['fresh','existing','nested','ancestor-history']:rows.append(dict(case=n,original=state(out/(n+'-original.igz')),native=state(out/(n+'-native.igz'))))
(out/'survey-reopens.json').write_text(json.dumps(rows,indent=2)+'\n')
