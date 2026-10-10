#!/usr/bin/env python3
"""Original actual caller and Text painter for a genuine dialog field atlas."""
import os,sys,json,copy
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPainter,QPdfWriter,QPageSize
from PySide6.QtCore import QSizeF
from core.composition import Composicion,TextoItem,field_values,expand_fields,RENDER_DPI
from views.composer import ComposerWindow,paint_text_mm
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
text='{hoja}/{total} {archivo} {nombre} {unknown}'
a=Composicion(name='Same',landscape=False,texts=[TextoItem(text=text,size_pt=14)])
b=copy.deepcopy(a);c=copy.deepcopy(a);c.name='Third'
a.border=True;a.border_color='#ffffff';a.border=False
comps=[a,b,c];scene=SimpleNamespace(compositions=comps)
caller=SimpleNamespace(_scene=lambda:scene,_window=SimpleNamespace(_current_path='/tmp/dynamic-fields.igz'))
rows=[]
for i,comp in enumerate(comps):
 ComposerWindow._set_field_context(caller,comp)
 assert field_values()['hoja']==str(i+1)
 name='field-'+str(i+1)
 writer=QPdfWriter(str(out/(name+'.original.pdf')));writer.setResolution(RENDER_DPI);writer.setPageSize(QPageSize(QSizeF(210,297),QPageSize.Unit.Millimeter))
 painter=QPainter(writer);painter.scale(RENDER_DPI/25.4,RENDER_DPI/25.4);painter.translate(20,30);paint_text_mm(painter,comp.texts[0]);painter.end()
 rows.append({'name':name,'sheet_name':comp.name,'authored_text':text,'properties':{},'expanded':expand_fields(text),'source_fields':field_values()})
(out/'source.json').write_text(json.dumps({'current_path':'/tmp/dynamic-fields.igz','cases':rows,'source':'actual ComposerWindow._set_field_context and paint_text_mm','runtime_first_border':{'on':False,'color':'#ffffff'},'scope':'Frozen source counterpart after native real property editing; no claim of original property pointer replay'},indent=2)+'\n')
print('PASS 3 original caller/painter field pages')
