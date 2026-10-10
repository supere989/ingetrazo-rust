#!/usr/bin/env python3
"""Operative source dormant border equality versus native-save lifetime."""
import os,sys,json,copy
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from core.composition import Composicion,field_values
from views.composer import ComposerWindow
app=QApplication([])
a=Composicion(name='Same');b=copy.deepcopy(a)
b.border=True;b.border_color='#ffffff';b.border=False
scene=SimpleNamespace(compositions=[a,b])
caller=SimpleNamespace(_scene=lambda:scene,_window=SimpleNamespace(_current_path=None))
ComposerWindow._set_field_context(caller,b)
assert field_values()['hoja']=='2' and a!=b
raw=b.to_dict();assert 'border' not in raw
c=Composicion.from_dict(raw);assert c.border_color=='#1e242c' and c==a
foreign=copy.deepcopy(raw);foreign['border']={'on':False,'color':'#ffffff'}
assert Composicion.from_dict(foreign)==a
report={'source':'actual Composicion.to_dict/from_dict and ComposerWindow._set_field_context','disabled_runtime_color':b.border_color,'disabled_runtime_equal':a==b,'runtime_hoja':field_values()['hoja'],'saved_border_present':'border' in raw,'reopened_color':c.border_color,'reopened_equal':c==a,'inactive_archive_ignored':True}
out=Path(sys.argv[1]);out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
