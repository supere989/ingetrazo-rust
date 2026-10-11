#!/usr/bin/env python3
"""Actual pinned source _light_vp and Qt matrices; no rendering acceptance."""
import ast,json,sys,hashlib,subprocess
from pathlib import Path
from PySide6.QtGui import QVector3D,QMatrix4x4
source=Path('/work/ingetrazo');pin=subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9'
p=source/'views/viewport.py';tree=ast.parse(p.read_text());method=next(n for c in tree.body if isinstance(c,ast.ClassDef) for n in c.body if isinstance(n,ast.FunctionDef) and n.name=='_light_vp');method.decorator_list=[]
ns=dict(QVector3D=QVector3D,QMatrix4x4=QMatrix4x4);exec(compile(ast.Module(body=[method],type_ignores=[]),str(p),'exec'),ns)
rows=[]
for bounds in (([-1,-2,0],[1,2,3]),([-.001,-.002,-.003],[.005,.006,.007]),([-10000,-1000,-50],[20000,1500,300]),([4.125,-9.75,2.5],[4.125,-9.75,2.5])):
 for direction in ((.3,.4,.8660254037844386),(.1,.1,.99),(0,0,1),(-.9,.2,.1),(0,1,0)):
  lo,hi=map(lambda v:QVector3D(*v),bounds);matrix=ns['_light_vp'](direction,lo,hi)
  rows.append(dict(direction=direction,lo=[lo.x(),lo.y(),lo.z()],hi=[hi.x(),hi.y(),hi.z()],matrix=list(matrix.data())))
Path(sys.argv[1]).write_text(json.dumps(dict(source_pin=pin,source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),rows=rows),indent=2)+'\n')
