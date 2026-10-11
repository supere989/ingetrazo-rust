#!/usr/bin/env python3
"""Actual pinned IGZ reader, existing documents only; no source controller recapture."""
import ast,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QMatrix4x4 as M
from core.scene import Scene
from formats import igz
app=QApplication([])
module=ast.parse(Path('scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in ['vec','mesh_state','shape']],type_ignores=[]),'existing-source-observers','exec'))
module=ast.parse(Path('scripts/reference-obj-fusion.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name=='flags'],type_ignores=[]),'flags-observer','exec'))
root=Path(sys.argv[1]);cases=json.loads((root/'original-obj-fusion.json').read_text())['cases'];rows=[]
for case in cases:
 row={'case':case['case']}
 for suffix in ['original','native']:
  scene=Scene();igz.load_into(scene,root/(case['case']+'-'+suffix+'.igz'));row[suffix]={'scene':shape(scene),'flags':flags(scene)}
 rows.append(row)
(root/'reopens-obj-fusion.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'actual_original_reopens':len(rows)}))
