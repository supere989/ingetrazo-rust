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
root=Path(sys.argv[1]);source=json.loads((root/'original-nested-import.json').read_text());rows=[]
for case in source['cases']:
 name=case['name'];pair={}
 for suffix in ['original','native']:
  s=Scene();igz.load_scene(s,root/f'{name}-{suffix}.igz');pair[suffix]=shape(s)
  meshes=[]
  def walk(groups,path=()):
   for i,g in enumerate(groups):
    p=path+(i,);meshes.append((p,g.mesh));walk(g.children,p)
  walk(s.groups)
  pair[suffix]['shared_mesh_pairs']=[[list(a),list(b)]for i,(a,ma)in enumerate(meshes)for b,mb in meshes[i+1:]if ma is mb]
 rows.append({'case':name,**pair})
(root/'reopens.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'actual_original_reopens':len(rows)}))
