#!/usr/bin/env python3
"""Actual original QGraphics TextItem/cache painter versus native canvas pixels.
Owned QImage proof at observed native paper transform; full original widget gesture
and cross-platform canvas behavior are separate gates.
"""
import os,sys,json,math,hashlib
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QGraphicsScene
from PySide6.QtGui import QImage,QPainter,QFont,QFontInfo
from PySide6.QtCore import QRectF,Qt
from core.composition import TextoItem
from views.composer import TextItem,ComposerCanvasView,ComposerWindow
app=QApplication([]);native=Path(sys.argv[1]);source=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
report=json.loads((native/'report.json').read_text());cases=json.loads(source.read_text())['cases'];rows=[];failed=False
for index,case in enumerate(cases):
 state=next(s for s in report['border_gestures']['canvas_states']if s['active_sheet']==index);a=QImage(str(native/('canvas-sheet-'+str(index+1)+'.png')));assert not a.isNull();x,y,z=state['paper_transform']
 scene=QGraphicsScene();owner=SimpleNamespace(TOOLS=ComposerWindow.TOOLS);view=ComposerCanvasView(scene,owner);view.setRenderHints(QPainter.Antialiasing|QPainter.SmoothPixmapTransform)
 items=[(case,30)]+([(case['secondary'],50)]if case.get('secondary')else [])
 for entry,pos_y in items:
  properties=entry.get('properties',{});model=TextoItem(text=entry['expanded'],x_mm=20,y_mm=pos_y,size_pt=14,**properties);scene.addItem(TextItem(owner,model))
 b=QImage(a.size(),QImage.Format_RGBA8888);b.fill(Qt.white);p=QPainter(b);p.setRenderHints(view.renderHints());p.translate(x,y);p.scale(z,z);scene.render(p,QRectF(0,0,a.width(),a.height()),QRectF(0,0,a.width(),a.height()),Qt.IgnoreAspectRatio);p.end();b.save(str(out/('original-'+str(index+1)+'.png')))
 for entry,pos_y in items:
  props=entry.get('properties',{});width=props.get('w_mm',80);height=14*25.4/72*1.35*(entry['expanded'].count('\n')+3)
  left=math.floor(x+19*z);top=math.floor(y+(pos_y-2)*z);right=math.ceil(x+(21+width)*z);bottom=math.ceil(y+(pos_y+height+2)*z);assert 0<left<right<a.width()and 0<top<bottom<a.height()
  changed=sum(a.pixelColor(i,j).rgba()!=b.pixelColor(i,j).rgba()for i in range(left,right)for j in range(top,bottom));ink=sum(min(b.pixelColor(i,j).red(),b.pixelColor(i,j).green(),b.pixelColor(i,j).blue())<240 for i in range(left,right)for j in range(top,bottom));assert ink>20
  rows.append({'sheet':index,'position_y_mm':pos_y,'roi':[left,top,right,bottom],'changed_pixels':changed,'original_ink_pixels':ink,'render_hints':view.renderHints().value});failed|=changed!=0
result={'passed':not failed,'rows':rows,'native_binary_sha256':report['binary_sha256'],'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','checker_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':__doc__};(out/'report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert not failed,'Original canvas TextItem pixel mismatch retained'
