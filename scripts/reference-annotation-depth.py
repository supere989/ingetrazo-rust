#!/usr/bin/env python3
"""Pinned original depth visibility math on controlled depth arrays, not GPU resolve proof."""
import ast, hashlib, json, subprocess, sys
from pathlib import Path
from types import SimpleNamespace
import numpy as np
from PySide6.QtGui import QMatrix4x4, QVector3D, QVector4D
source, output = map(Path,sys.argv[1:3])
pin="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git","-C",str(source),"rev-parse","HEAD"],text=True).strip()==pin
path=source/"views/viewport.py";tree=ast.parse(path.read_text())
method=next(n for c in tree.body if isinstance(c,ast.ClassDef) for n in c.body if isinstance(n,ast.FunctionDef) and n.name=="_occluded_on_screen")
exec(compile(ast.fix_missing_locations(ast.Module(body=[method],type_ignores=[])),str(path),"exec"))
rows=[]
def case(name, world, depth=.5, patches=(), eye=(0,0,0), fwd=(0,0,1), matrix=None, fallback=None):
    buf=np.full((5,5),depth,dtype=np.float32)
    for y,x,d in patches:buf[y,x]=d
    m=QMatrix4x4() if matrix is None else matrix
    inv,ok=m.inverted();assert ok
    calls=[]
    def ray(w):calls.append(list(w.toTuple()));return bool(fallback)
    view=SimpleNamespace(_depth_snap=None if fallback is not None else (buf,m,inv,5,5,QVector3D(*eye),QVector3D(*fwd)),_is_occluded=ray)
    result=bool(_occluded_on_screen(view,QVector3D(*world)))
    rows.append(dict(name=name,world=world,depth=buf.tolist(),mvp_column_major=list(m.data()),inverse_mvp_column_major=list(inv.data()),eye=eye,forward=fwd,fallback=fallback,fallback_calls=calls,occluded=result))
case("surface",(0,0,0))
case("within-absolute-tolerance",(0,0,.019))
case("beyond-absolute-tolerance",(0,0,.021))
case("background",(0,0,.5),1.)
case("neighbor-background",(0,0,.5),patches=[(1,1,1.)])
case("outside-neighborhood-background",(0,0,.5),patches=[(0,0,1.)])
case("farthest-neighbor",(0,0,.2),patches=[(1,1,.7)])
case("clipped-corner-neighborhood",(-1,-1,.2),patches=[(1,1,.7)])
case("negative-fraction-truncates-to-zero",(-1.1,0,.2))
case("negative-outside",(-1.5,0,.2))
case("positive-boundary-outside",(1,0,.2))
case("translated-eye-forward-distance",(0,0,100),eye=(0,0,99.8),patches=[(2,2,.5)])
case("far-relative-tolerance-visible",(0,0,.2),eye=(0,0,-100))
case("far-relative-tolerance-hidden",(0,0,.4),eye=(0,0,-100))
case("missing-depth-ray-hidden",(0,0,.2),fallback=True)
case("missing-depth-ray-visible",(0,0,.2),fallback=False)
p=QMatrix4x4();p.perspective(60.,1.,.1,100.)
case("perspective-behind-eye",(0,0,1),matrix=p,fwd=(0,0,-1))
case("perspective-front",(0,0,-2),matrix=p,fwd=(0,0,-1))
by={r["name"]:r for r in rows}
assert not by["within-absolute-tolerance"]["occluded"] and by["beyond-absolute-tolerance"]["occluded"]
assert not by["neighbor-background"]["occluded"] and by["outside-neighborhood-background"]["occluded"]
assert by["negative-fraction-truncates-to-zero"]["occluded"] and not by["negative-outside"]["occluded"]
assert not by["far-relative-tolerance-visible"]["occluded"] and by["far-relative-tolerance-hidden"]["occluded"]
assert not by["perspective-behind-eye"]["occluded"] and by["perspective-front"]["occluded"]
assert all(len(r["fallback_calls"])==(1 if r["fallback"] is not None else 0) for r in rows)
output.parent.mkdir(parents=True,exist_ok=True)
output.write_text(json.dumps(dict(source_pin=pin,source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),scope="Actual original method with synthetic depth; GPU MSAA resolve and live viewport remain unverified",cases=rows),indent=2)+"\n")
print(json.dumps({"cases":len(rows),"occluded":sum(r["occluded"] for r in rows)}))
