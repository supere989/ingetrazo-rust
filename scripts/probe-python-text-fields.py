#!/usr/bin/env python3
"""Actual source dynamic field recipes, including bound frame and source :g formatting."""
import sys,json,datetime,os
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from core.composition import Composicion,Cajetin,MarcoVista,set_field_context,field_values,expand_fields,format_scale
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
frames=[dict(uid='small',w_mm=40,h_mm=30,scale_n=.1,view_key='scene:Detail'),dict(uid='main',w_mm=100,h_mm=100,scale_n=123.456789,view_key='std:top')]
recipes=[
 dict(name='empty',sheet=None),
 dict(name='main',sheet='Sheet A',frames=frames,title=dict(proyecto='House',autor='Author',lamina='L-07'),index=1,total=4,path='/tmp/House.rev2.igz'),
 dict(name='bound',sheet='Sheet A',frames=frames,frame_uid='small'),
 dict(name='missing_bound',sheet='Sheet A',frames=frames,frame_uid='missing'),
 dict(name='first_tie',sheet='Tie',frames=[dict(uid='one',w_mm=10,h_mm=20,scale_n=50,view_key='scene:First'),dict(uid='two',w_mm=20,h_mm=10,scale_n=20,view_key='scene:Second')]),
 dict(name='empty_title',sheet='Fallback sheet',title=dict(lamina='')),
 dict(name='mixed_placeholders',sheet='Ångle',text='{NOMBRE} {unknown} {n} {fecha} {hoja} {total} {nombre_} {Nómbre} {{nombre}}',index=0,total=0),
 dict(name='hidden_bound',sheet='Hidden',frames=[dict(uid='hide',w_mm=5,h_mm=5,scale_n=.00012345678,view_key='scene:Hidden',hidden=True)],frame_uid='hide'),
 dict(name='unicode_path',sheet='Plan',path='/tmp/café.drawing.IGZ'),
]
rows=[]
for r in recipes:
 comp=None if r['sheet'] is None else Composicion(name=r['sheet'],frames=[MarcoVista(**v)for v in r.get('frames',[])],cajetin=Cajetin(**r['title'])if 'title'in r else None)
 set_field_context(comp=comp,path=r.get('path'),index=r.get('index'),total=r.get('total'))
 text=r.get('text','{proyecto}|{autor}|{lamina}|{nombre}|{escala}|{escena}|{fecha}|{archivo}|{hoja}|{total}')
 rows.append(dict(recipe=r,text=text,source_values=field_values(r.get('frame_uid','')),expanded=expand_fields(text,r.get('frame_uid',''))))
scales=[-1,0,.1,.3,.00012345678,1,50,123.456789,999999.5,1000000,1e20,1e-20,1.000005,1.234565,1.234575]
report=dict(source='core.composition.field_values/expand_fields/format_scale actual operative functions',date=datetime.date.today().isoformat(),cases=rows,scales=[dict(value=v,formatted=format_scale(v))for v in scales],scope='Source dynamic field baseline only; native PDF/Text UI still required')
# Actual helper PDFs with context matching a real selected export: index and total describe real sheets.
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPainter,QPdfWriter,QPageSize
from PySide6.QtCore import QSizeF
from core.composition import TextoItem,RENDER_DPI
from views.composer import paint_text_mm
app=QApplication([]);pdf_cases=[]
for row in rows:
 r=row['recipe']
 if r['sheet'] is None:continue
 index=r.get('index',0);total=max(r.get('total',1),index+1)
 comp=Composicion(name=r['sheet'],frames=[MarcoVista(**v)for v in r.get('frames',[])],cajetin=Cajetin(**r['title'])if 'title'in r else None)
 set_field_context(comp=comp,path=r.get('path'),index=index,total=total)
 item=TextoItem(text=row['text'],size_pt=14,frame_uid=r.get('frame_uid',''))
 writer=QPdfWriter(str(out/(r['name']+'.original.pdf')));writer.setResolution(RENDER_DPI);writer.setPageSize(QPageSize(QSizeF(210,297),QPageSize.Unit.Millimeter));painter=QPainter(writer);painter.scale(RENDER_DPI/25.4,RENDER_DPI/25.4);painter.translate(20,30);paint_text_mm(painter,item);painter.end()
 pdf_cases.append(dict(recipe=r,text=row['text'],index=index,total=total,expanded=expand_fields(row['text'],r.get('frame_uid',''))))
report['pdf_cases']=pdf_cases
(out/'source.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('PASS',len(rows),'source field contexts',len(scales),'source scale values')
