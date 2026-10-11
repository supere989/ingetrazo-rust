#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.units import UNIT_CHOICES
from formats import igz
app=QApplication([]);p=Path(sys.argv[1]);rows=[]
def raw(path):
 import zipfile
 if zipfile.is_zipfile(path):
  with zipfile.ZipFile(path) as z:return json.loads(z.read('document.json'))['scene']['units']
 return json.loads(path.read_text())['scene']['units']
assert raw(p/'vendor-noop-native.igz')==raw(p/'vendor-input.igz')
assert raw(p/'away-back-native.igz')=={'length':'ft-in','precision':4}
for code in UNIT_CHOICES:
 assert raw(p/(code+'.igz'))==raw(p/(code+'-native.igz')),('raw units',code)
 values=[]
 for suffix in ['', '-native']:
  s=Scene();igz.load_into(s,p/(code+suffix+'.igz'));values.append({'units':s.units,'style':s.dimension_style})
 assert values[0]==values[1],(code,values)
 rows.append({'case':code,'units':values[1]['units'],'style':values[1]['style']})
(p/'reopen-units.json').write_text(json.dumps({'cases':rows},indent=2));print('eight actual original unit/style reopens exact')
