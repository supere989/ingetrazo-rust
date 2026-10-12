#!/usr/bin/env python3
"""Actual unchanged original reader on retained and newly native-saved documents."""
import ast,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
for filename,names in [('reference-nested-import.py',['vec','mesh_state','shape']),('reference-dxf-curves.py',['curves']),('reference-dxf-blocks.py',['owners'])]:
 tree=ast.parse(Path('scripts',filename).read_text());exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names],type_ignores=[]),filename,'exec'))
app=QApplication([]);root=Path(sys.argv[1]);tag='-addendum' if '--addendum' in sys.argv else '';source=json.loads((root/('original-dxf-blocks-addendum-all.json' if tag else 'original-dxf-blocks.json')).read_text());rows=[]
for case in source['cases']:
 name=case['name'];values=[]
 for suffix in ['original','native']:
  s=Scene();igz.load_into(s,root/(name+'-'+suffix+'.igz'));values.append(dict(scene=shape(s),curves=curves(s),owners=owners(s)))
 rows.append(dict(case=name,original=values[0],native=values[1]))
if not tag:
 original=Scene();native=Scene();igz.load_into(original,root/'shared-base-original.igz');igz.load_into(native,root/'cancel-native.igz')
 (root/'cancel-block-reopen.json').write_text(json.dumps(dict(original=dict(scene=shape(original),curves=curves(original),owners=owners(original)),native=dict(scene=shape(native),curves=curves(native),owners=owners(native))),indent=2)+'\n')
(root/('reopens-dxf-blocks'+tag+'.json')).write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(dict(actual_original_reopens=len(rows))))
