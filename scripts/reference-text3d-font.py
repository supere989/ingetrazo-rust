#!/usr/bin/env python3
"""Read-only original 3D Text outlines and mesh inventory, no native claims."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QFontInfo,QFontMetricsF,QPainterPath
from PySide6.QtCore import qVersion
sys.path.insert(0,'/work/ingetrazo')
from core import text3d
app=QApplication([])
rows=[]
for name,text,bold,italic,height,thickness in [
 ('holes','OB8',False,False,.25,.05),('concave','LMW',False,False,.25,.05),
 ('kerning','AV To',False,False,.25,.05),('descenders','gyp',False,False,.25,.05),
 ('spaces',' A  V ',False,False,.25,.05),('flat','O',False,False,.25,0),
 ('large','Bg',False,False,20,2),('bold','Bgy',True,False,.25,.05),
 ('italic','AVgy',False,True,.25,.05),('bold-italic','OB',True,True,.25,.05)]:
 font=text3d._font('DejaVu Sans',bold,italic)
 info=QFontInfo(font);fm=QFontMetricsF(font)
 rings,_=text3d._rings(text,'DejaVu Sans',bold,italic)
 letters=text3d.build_text_letters(text,'DejaVu Sans',bold,italic,height,thickness)
 glyphs=[]
 for i,ch in enumerate(text):
  if ch.isspace():continue
  x=fm.horizontalAdvance(text[:i]) if i else 0
  rr,_=text3d._rings(ch,'DejaVu Sans',bold,italic,x)
  glyphs.append({'char':ch,'advance':x,'rings':rr})
 meshes=[]
 from types import SimpleNamespace
 kids=[]
 for ch,m in letters:
  kids.append(SimpleNamespace(name=ch,mesh=m))
  vs=list(m.vertices);index={id(v):i for i,v in enumerate(vs)}
  meshes.append({'char':ch,'vertices':[[v.position.x(),v.position.y(),v.position.z()] for v in vs],
   'faces':[{'outer':[index[id(v)] for v in f.loop],'holes':[[index[id(v)] for v in h] for h in f.hole_loops]} for f in m.faces],
   'edges':[{'vertices':[index[id(e.v0)],index[id(e.v1)]],'soft':e.soft} for e in m.edges]})
 rows.append({'name':name,'params':{'text':text,'font':'DejaVu Sans','bold':bold,'italic':italic,'height':height,'thickness':thickness},'font':{'family':info.family(),'pixel_size':info.pixelSize(),'point_size':info.pointSizeF()},'rings':rings,'glyphs':glyphs,'meshes':meshes,'fingerprint':text3d.letters_fingerprint(kids)})
print(json.dumps({'source':subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip(),'qt':qVersion(),'dpi':app.primaryScreen().logicalDotsPerInch(),'cases':rows},separators=(',',':')))
