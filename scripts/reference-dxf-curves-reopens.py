#!/usr/bin/env python3
"""Read frozen original saves and newly authored native saves; no handler recapture."""
import ast,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QMatrix4x4 as M
from core.scene import Scene
from formats import igz
app=QApplication([])
module=ast.parse(Path('scripts/reference-nested-import.py').read_text())
functions=ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in ['vec','mesh_state','shape']],type_ignores=[])
exec(compile(functions,'frozen-source-observation','exec'))
root=Path(sys.argv[1]);source=json.loads((root/'original-dxf-curves.json').read_text());rows=[]
for case in source['cases']:
 name=case['name'];pair={}
 for suffix in ['original','native']:
  s=Scene();igz.load_into(s,root/f'{name}-{suffix}.igz');pair[suffix]=shape(s)
  meshes=[];kinds=[];entity_layers=[]
  def layers(mesh,path):
   for e in mesh.edges:
    entity_layers.append({'path':list(path),'kind':'edge','points':sorted([vec(e.a),vec(e.b)]),'layer':getattr(e,'layer',None)})
   for f in mesh.faces:
    entity_layers.append({'path':list(path),'kind':'face','points':sorted(vec(v)for v in f.vertices),'layer':getattr(f,'layer',None)})
  layers(s.mesh,())
  def walk(groups,path=()):
   for i,g in enumerate(groups):
    p=path+(i,);meshes.append((p,g.mesh));layers(g.mesh,p)
    kinds.append({'path':list(p),'name':g.name,'component':g.component,'is_component':g.is_component(),'xform':None if g.xform is None else list(g.xform.data())})
    walk(g.children,p)
  walk(s.groups)
  pair[suffix]['group_kinds']=kinds
  pair[suffix]['entity_layers']=sorted(entity_layers,key=lambda row:json.dumps(row,sort_keys=True))
  pair[suffix]['shared_mesh_pairs']=[[list(a),list(b)]for i,(a,ma)in enumerate(meshes)for b,mb in meshes[i+1:]if ma is mb]
 rows.append({'case':name,**pair})
(root/'reopens-dxf-curves.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'actual_original_reopens':len(rows)}))
