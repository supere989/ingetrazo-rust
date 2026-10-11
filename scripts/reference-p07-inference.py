#!/usr/bin/env python3
"""Capture only missing P07 guide/pixel/Unicode contracts from the fixed source."""
import argparse,hashlib,json,math,os,subprocess,sys,unicodedata
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QVector4D as V4,QMatrix4x4
from core.guide import Guide
from core.snap import compute_snap
from core import units
from views.main_window import MainWindow
from views.viewport import Viewport
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def array(p):return[p.x(),p.y(),p.z()]
def matrix(m):return list(m.data())
def model(s):return sorted([sorted([array(e.a),array(e.b)])for e in s.mesh.edges])
def main():
 ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args();assert not a.output.exists();a.output.mkdir();assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN
 app=QApplication([]);win=MainWindow();vp=win.viewport;vp.resize(1000,800);rows=[]
 center=V(3,2,1)
 views=[('xy',V(1,0,0),V(0,1,0)),('xz',V(1,0,0),V(0,0,1)),('yz',V(0,1,0),V(0,0,1)),('oblique',V(1,-1,0).normalized(),V(1,1,-2).normalized())]
 for view,u,v in views:
  normal=V.crossProduct(u,v).normalized();m=QMatrix4x4(u.x()*.2,u.y()*.2,u.z()*.2,-V.dotProduct(u,center)*.2,v.x()*.25,v.y()*.25,v.z()*.25,-V.dotProduct(v,center)*.25,-normal.x()*.01,-normal.y()*.01,-normal.z()*.01,.5+V.dotProduct(normal,center)*.01,0,0,0,1)
  camera=N(projection_matrix=lambda:m,view_matrix=lambda:QMatrix4x4());stub=N(camera=camera)
  def project(p):
   c=m.map(V4(p.x(),p.y(),p.z(),1));return(500+500*c.x()/c.w(),400-400*c.y()/c.w())if c.w()>0 else None
  specs=[('cross',0),('inside14',13.999),('exact14',14),('outside14',14.001),('skew',0),('parallel',0),('occluded',0),('guide-model',0),('point-inside9',8.999),('point-exact9',9),('point-outside9',9.001),('point-priority',0)]
  for label,offset in specs:
   point_mode=label.startswith('point-');guides=[Guide(center)]if point_mode else[Guide(center,u),Guide(center+normal if label=='skew'else center,u if label=='parallel'else v)]
   edges=[];segments=[]
   for g in guides:
    if g.is_line:
     seg=Viewport._guide_snap_segment(stub,g)
     if seg:edges.append(N(a=seg[0],b=seg[1],center=False,guide=True));segments.append([array(seg[0]),array(seg[1])])
    else:edges.append(N(a=g.point,b=g.point,center=False));segments.append([array(g.point),array(g.point)])
   model_edges=[]
   if label=='guide-model':
    g=guides.pop();edges.pop();segments.pop();model_edges=[[array(center-v),array(center+v)]];edges.insert(0,N(a=center-v,b=center+v,center=False))
   px=project(center);cursor=[px[0]+offset,px[1]];candidate=center+u*(offset/100)
   r=compute_snap(candidate,cursor,N(edges=edges),project,threshold_px=9,edge_threshold_px=14,is_occluded=(lambda p:(p-center).length()<.5)if label=='occluded'else None)
   rows.append({'name':view+'-'+label,'matrix':matrix(m),'center':array(center),'normal':array(normal),'u':array(u),'v':array(v),'guides':[{'point':array(g.point),'direction':array(g.direction)if g.direction is not None else None}for g in guides],'segments':segments,'edges':model_edges,'candidate':array(candidate),'cursor':cursor,'occluded':label=='occluded','source':{'kind':r.kind,'point':array(r.point)}})
 # Capture the full decimal digit law once, not a representative easy alphabet.
 starts=[i for i in range(0x110000)if unicodedata.category(chr(i))=='Nd'and unicodedata.decimal(chr(i))==0];numeric=[]
 for zero in starts:
  for ascii_text,comma in [('12.5mm',False),('2,5;1,2',True),('1 1/2"',False),('3:12',False)]:
   text=''.join(chr(zero+int(c))if c.isascii()and c.isdigit()else c for c in ascii_text)
   with units.unitless_numbers():value=Viewport._parse_value_buffer(text,comma_lists=comma)
   if isinstance(value,tuple)and value[0]=='ratio':value=value[1]
   numeric.append({'text':text,'mode':'angle'if':'in text else'fields','comma_lists':comma,'value':list(value)if isinstance(value,tuple)else[value]if isinstance(value,float)else None})
 for text in ['١٫٥','１．５','²','⅓','١٬٥','١e٣','+١','١\x1c٢','١\x1f٢','١ ٢','١ ٢','١,٢','١,٢;٣,٤','١, ٢']:
  value=Viewport._parse_value_buffer(text,comma_lists=True);numeric.append({'text':text,'mode':'fields','comma_lists':True,'value':list(value)if isinstance(value,tuple)else[value]if isinstance(value,float)else None})
 files=['core/snap.py','core/guide.py','core/topology.py','views/viewport.py','core/units.py'];report={'source_pin':PIN,'source_files':{f:hashlib.sha256((Path('/work/ingetrazo')/f).read_bytes()).hexdigest()for f in files},'unicode_version':unicodedata.unidata_version,'decimal_zeroes':starts,'cases':rows,'numeric':numeric,'scope':'Actual original guide clipping and compute_snap with fixed viewport matrices; logical pixel boundary/three-dimensional/analytic occlusion contracts. Full Unicode decimal grammar via actual Viewport parser. No rendered pixels.'}
 (a.output/'source.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n');print(json.dumps({'cases':len(rows),'numeric':len(numeric)}));win._saved_version=vp.scene.version;win.close()
if __name__=='__main__':main()
