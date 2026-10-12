#!/usr/bin/env python3
"""Alternate Qt public stroker outline; equivalence to actual paint is mandatory."""
import hashlib,json,math,os,subprocess,sys
from pathlib import Path
source=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True)
repo=Path(__file__).resolve().parents[1];pin="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(['git','-C',str((repo/'ingetrazo').resolve()),'rev-parse','HEAD'],text=True).strip()==pin
os.environ['QT_QPA_PLATFORM']='offscreen'
from PySide6.QtCore import Qt,qVersion
from PySide6.QtGui import QImage,QPainter,QPainterPath,QPainterPathStroker,QColor,QTransform
from PySide6.QtWidgets import QApplication
app=QApplication([]);assert qVersion()=='6.12.0'
record=json.loads((source/'original-callout-coverage.json').read_text());assert record['source_pin']==pin and record['case']=='llamadas-default'
st=record['stages'][0];assert not st['has_clip'] and not st['view_transform_enabled']
assert st['size_mode']==Qt.AbsoluteSize.value and st['device']['pixel_ratio']==1
assert all(math.isfinite(v) for v in st['world_transform']) and st['world_transform'][2]==st['world_transform'][5]==0 and st['world_transform'][8]==1
assert all(math.isfinite(v) for v in st['rect']+st['radii']) and all(v>0 for v in st['rect'][2:]+st['radii'])
assert st['opacity']==1 and st['composition']==0 and st['pen']['rgba'][3]==255
scale=st['world_transform'][0];assert scale>0 and st['world_transform'][4]==scale and st['world_transform'][1]==st['world_transform'][3]==0
x,y,w,h=st['rect'];rx=min(st['radii'][0],w/2);ry=min(st['radii'][1],h/2);k=.5522847498
# Literal QPaintEngineEx::drawRoundedRect local17-point program.
ops=[('M',[x+rx,y]),('L',[x+w-rx,y]),('C',[x+w-(1-k)*rx,y,x+w,y+(1-k)*ry,x+w,y+ry]),('L',[x+w,y+h-ry]),('C',[x+w,y+h-(1-k)*ry,x+w-(1-k)*rx,y+h,x+w-rx,y+h]),('L',[x+rx,y+h]),('C',[x+(1-k)*rx,y+h,x,y+h-(1-k)*ry,x,y+h-ry]),('L',[x,y+ry]),('C',[x,y+(1-k)*ry,x+(1-k)*rx,y,x+rx,y])]
def flatten(q,threshold):
 stack=[(q,9)];points=[]
 while stack:
  q,level=stack.pop();dx=q[3][0]-q[0][0];dy=q[3][1]-q[0][1];length=abs(dx)+abs(dy)
  if length>1:deviation=abs(dx*(q[0][1]-q[1][1])-dy*(q[0][0]-q[1][0]))+abs(dx*(q[0][1]-q[2][1])-dy*(q[0][0]-q[2][0]))
  else:deviation=abs(q[0][0]-q[1][0])+abs(q[0][1]-q[1][1])+abs(q[0][0]-q[2][0])+abs(q[0][1]-q[2][1]);length=1
  if deviation<threshold*length or level==0:points.append(q[3]);continue
  mid=lambda a,b:((a[0]+b[0])*.5,(a[1]+b[1])*.5)
  ab=mid(q[0],q[1]);bc=mid(q[1],q[2]);cd=mid(q[2],q[3]);abc=mid(ab,bc);bcd=mid(bc,cd);middle=mid(abc,bcd)
  stack.extend([([middle,bcd,cd,q[3]],level-1),([q[0],ab,abc,middle],level-1)])
 return points
path=QPainterPath();points=[]
for kind,data in ops:
 if kind=='M':current=tuple(data);path.moveTo(*current);points.append(current)
 elif kind=='L':current=tuple(data);path.lineTo(*current);points.append(current)
 else:
  end=tuple(data[4:]);part=flatten([current,tuple(data[:2]),tuple(data[2:4]),end],.5/scale)
  for point in part:path.lineTo(*point)
  points.extend(part);current=end
path.closeSubpath()
stroker=QPainterPathStroker();stroker.setWidth(st['pen']['width']);stroker.setCapStyle(Qt.PenCapStyle(st['pen']['cap']));stroker.setJoinStyle(Qt.PenJoinStyle(st['pen']['join']));stroker.setMiterLimit(st['pen']['miter']);stroker.setDashPattern(st['pen']['dash_pattern']);stroker.setDashOffset(st['pen']['dash_offset'])
outline=stroker.createStroke(path)
size=st['transparent']['size'];image=QImage(*size,QImage.Format_ARGB32_Premultiplied);image.fill(QColor(0,0,0,0));image.setDotsPerMeterX(st['device']['dpm'][0]);image.setDotsPerMeterY(st['device']['dpm'][1]);image.setDevicePixelRatio(st['device']['pixel_ratio'])
paint=QPainter(image);paint.setRenderHints(paint.renderHints(),False);paint.setRenderHints(QPainter.RenderHint(st['hints']));paint.setWorldTransform(QTransform(*st['world_transform']));paint.setPen(Qt.NoPen);paint.setBrush(QColor(*st['pen']['rgba']));t=paint.combinedTransform();assert [t.m11(),t.m12(),t.m13(),t.m21(),t.m22(),t.m23(),t.m31(),t.m32(),t.m33()]==st['combined_transform'] and paint.renderHints().value==st['hints'];paint.drawPath(outline);paint.end()
def raw(image):
 image=image.convertToFormat(QImage.Format_RGBA8888);data=bytes(image.constBits());return b''.join(data[j*image.bytesPerLine():j*image.bytesPerLine()+image.width()*4]for j in range(image.height()))
actual=(source/st['transparent']['rgba']).read_bytes();assert hashlib.sha256(actual).hexdigest()==st['transparent']['rgba_sha256']
probe=raw(image);assert len(probe)==len(actual);equivalent=probe==actual
assert image.save(str(out/'outline.png'));(out/'outline.rgba').write_bytes(probe)
result={'source_pin':pin,'case':'llamadas-default','qt_version':qVersion(),'flatten_threshold':.5/scale,'local_polyline':points,'outline':[{'type':outline.elementAt(i).type.value,'xy':[outline.elementAt(i).x,outline.elementAt(i).y]}for i in range(outline.elementCount())],'full_mask_equivalent':equivalent,'different_rgba_pixels':sum(probe[i:i+4]!=actual[i:i+4]for i in range(0,len(probe),4)),'actual_rgba_sha256':hashlib.sha256(actual).hexdigest(),'outline_rgba_sha256':hashlib.sha256(probe).hexdigest(),'scope':'Alternate public Qt stroker on literal local flattened source path. Not private interception. Outline is an operative witness ONLY if full mask equality to independently captured actual drawRoundedRect holds.'}
(out/'original-callout-outline.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'equivalent':equivalent,'pixels':result['different_rgba_pixels']}))
