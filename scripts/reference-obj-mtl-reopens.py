#!/usr/bin/env python3
"""Actual original reader for frozen original and newly saved native material documents."""
import ast,hashlib,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QMatrix4x4 as M
from core.scene import Scene
from core.texture import texture_file_name
from formats import igz
app=QApplication([])
for helper,names in [('scripts/reference-nested-import.py',['vec','mesh_state','shape']),('scripts/reference-obj-mtl-import.py',['recipe'])]:
 module=ast.parse(Path(helper).read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in names],type_ignores=[]),'frozen-original-observer','exec'))
def normalized(value):
 if isinstance(value,dict):
  if 'attrs'in value:value['attrs']=recipe(value['attrs'])
  for key,item in list(value.items()):value[key]=normalized(item)
 elif isinstance(value,list):value=[normalized(v)for v in value]
 return value
root=Path(sys.argv[1]);original=json.loads((root/'original-obj-mtl.json').read_text());rows=[]
for case in original['cases']:
 name=case['case'];pair={}
 for suffix in ['original','native']:
  s=Scene();igz.load_into(s,root/(name+'-'+suffix+'.igz'));pair[suffix]=normalized(shape(s));meshes=[]
  def walk(groups,path=()):
   for i,g in enumerate(groups):
    p=path+(i,);meshes.append((p,g.mesh));walk(g.children,p)
  walk(s.groups);pair[suffix]['shared_mesh_pairs']=[[list(a),list(b)]for i,(a,ma)in enumerate(meshes)for b,mb in meshes[i+1:]if ma is mb]
 rows.append({'case':name,**pair})
(root/'reopens-obj-mtl.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'actual_original_reopens':len(rows)}))
