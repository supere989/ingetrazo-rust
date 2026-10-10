#!/usr/bin/env python3
"""Actual native glyph regions against independently source-expanded literal controls."""
import os,sys,json,math
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtGui import QImage
native,control,baseline=map(Path,sys.argv[1:]);report=json.loads((native/'report.json').read_text());literal=json.loads((control/'report.json').read_text())
rows=[]
for index in range(3):
 n=next(s for s in report['border_gestures']['canvas_states']if s['active_sheet']==index)
 c=next(s for s in literal['border_gestures']['canvas_states']if s['active_sheet']==index)
 assert n['canvas_text']==c['canvas_text'] and len(n['canvas_text_rects'])==len(c['canvas_text_rects'])==len(n['canvas_text']) and len(n['canvas_text'])>0
 a=QImage(str(native/('canvas-sheet-'+str(index+1)+'.png')));b=QImage(str(control/('canvas-sheet-'+str(index+1)+'.png')))
 assert not a.isNull()and not b.isNull()and a.size()==b.size()
 for item in range(len(n['canvas_text'])):
  rect=n['canvas_text_rects'][item];other=c['canvas_text_rects'][item];assert rect==other
  left,top=math.floor(rect[0])-3,math.floor(rect[1])-3;right,bottom=math.ceil(rect[2])+3,math.ceil(rect[3])+3
  assert 0<left<right<a.width() and 0<top<bottom<a.height()
  changed=sum(a.pixel(x,y)!=b.pixel(x,y)for x in range(left,right)for y in range(top,bottom));assert changed==0,(index,changed)
  ink=sum(max(a.pixelColor(x,y).red(),a.pixelColor(x,y).green(),a.pixelColor(x,y).blue())<180 for x in range(left,right)for y in range(top,bottom));assert ink>50,(index,ink)
  row={'sheet':index,'item':item,'actual_source_expanded_text':n['canvas_text'][item],'text_rect':rect,'exact_literal_control_changed_pixels':changed,'actual_glyph_dark_pixels':ink}
  if index==0 and item==0:
   wrong=QImage(str(baseline/'border-white-disabled.png'));assert not wrong.isNull()and wrong.size()==a.size()
   difference=sum(a.pixel(x,y)!=wrong.pixel(x,y)for x in range(left,right)for y in range(top,bottom));assert difference>0
   row['retained_literal_macro_failure_changed_pixels']=difference
  rows.append(row)
result={'passed':True,'cases':rows,'scope':'Actual native canvas glyph routing against actual original expanded strings rendered as independent literal controls; original canvas font/layout pixel parity remains open'}
(native/'canvas-pixel-reader.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
