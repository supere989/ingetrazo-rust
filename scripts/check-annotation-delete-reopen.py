#!/usr/bin/env python3
import sys,os,json,copy
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
p=Path(sys.argv[1]);app=QApplication([])
def point(v):return [v.x(),v.y(),v.z()]
def mesh(m):return {'vertices':[point(v.position) for v in m.vertices],'edges':[[point(e.a),point(e.b)] for e in m.edges],'faces':[{'outer':[point(v) for v in f.vertices],'holes':[[point(v) for v in h] for h in f.holes],'attrs':f.attrs} for f in m.faces]}
def group(g):return {'name':g.name,'mesh':mesh(g.mesh),'children':[group(c) for c in g.children],'xform':list(g.xform.data()) if g.xform is not None else None}
def read(file):
 s=Scene();igz.load_into(s,file);return {'mesh':mesh(s.mesh),'groups':[group(g) for g in s.groups],'guides':[g.to_dict() for g in s.guides],'dimensions':[{'a':point(d.a),'b':point(d.b),'offset':point(d.offset),'text':d.text,'axis':d.axis,'layer':d.layer} for d in s.dimensions],'labels':[t.to_dict() for t in s.text_labels]}
def validate(a,b):
 assert a==b, "document reopen differs"
observed={}
for case in ['root','nested-annotations','nested-mixed','parked']:
 a=read(p/(case+'-original.igz'));b=read(p/(case+'-native.igz'));validate(a,b);observed[case]=b
 bad=copy.deepcopy(b);bad['guides']=[]
 try: validate(a,bad)
 except AssertionError: pass
 else: raise AssertionError('lossy reopen accepted')
(p/'original-reopens.json').write_text(json.dumps(observed,indent=2));print(json.dumps({'original_reopens':4,'negative_controls':4,'exact':True}))
