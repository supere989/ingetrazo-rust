#!/usr/bin/env python3
"""Actual production ComposerWindow field context, including dataclass-equal sheets."""
import os,sys,json
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from pathlib import Path
from types import SimpleNamespace
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPdfWriter,QPainter,QPageSize
from PySide6.QtCore import QSizeF
from core.composition import RENDER_DPI
from views.composer import paint_text_mm
from core.composition import Composicion,TextoItem,MarcoVista,field_values
from views.composer import ComposerWindow
app=QApplication([]);rows=[]
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
for name,alter,frames in [('equal_text_only',False,False),('different_text_position',True,False),('different_frame_uid',False,True),('same_literal_frame_uid',False,True),('large_metadata_integer',False,False)]:
 a=Composicion(name='Same',landscape=False,texts=[TextoItem(text='{hoja}/{total}')])
 b=Composicion(name='Same',landscape=False,texts=[TextoItem(text='{hoja}/{total}',x_mm=21 if alter else 20)])
 if frames:a.frames=[MarcoVista(uid='first',hidden=True)];b.frames=[MarcoVista(uid='first' if name=='same_literal_frame_uid' else 'second',hidden=True)]
 if name=='large_metadata_integer':a.texts[0].z=9007199254740992;b.texts[0].z=9007199254740993
 scene=SimpleNamespace(compositions=[a,b]);caller=SimpleNamespace(_scene=lambda:scene,_window=SimpleNamespace(_current_path=None))
 ComposerWindow._set_field_context(caller,b)
 v=field_values();assert v['hoja']==('1' if name in ['equal_text_only','same_literal_frame_uid'] else '2')
 rows.append(dict(case=name,distinct_objects=a is not b,value_equal=a==b,physical_index=1,source_fields=v))
 writer=QPdfWriter(str(out/(name+'.original.pdf')));writer.setResolution(RENDER_DPI);writer.setPageSize(QPageSize(QSizeF(210,297),QPageSize.Unit.Millimeter));painter=QPainter(writer);painter.scale(RENDER_DPI/25.4,RENDER_DPI/25.4);painter.translate(b.texts[0].x_mm,b.texts[0].y_mm);paint_text_mm(painter,b.texts[0]);painter.end()
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);(out/'source-caller-index.json').write_text(json.dumps({'source':'views.composer.ComposerWindow._set_field_context actual production method','scope':'Original dataclass equality index semantics; native pointer-only draft remains a retained discrepancy','cases':rows},indent=2,ensure_ascii=False)+'\n');print('PASS five actual source caller controls')
