"""Actual retained PDF bearing-only mutation diagnostic, not production proof."""
import io,json,os,re,struct,tempfile,zipfile,sys,hashlib,zlib
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from pathlib import Path
from fontTools.ttLib import TTFont
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
import numpy as np
app=QApplication([]);z=zipfile.ZipFile(sys.argv[1])
def sfnt(pdf):
 for m in re.finditer(rb'stream\r?\n(.*?)\r?\nendstream',pdf,re.S):
  raw=m.group(1)
  try:raw=zlib.decompress(raw)
  except zlib.error:pass
  if raw[:4]==b'\x00\x01\x00\x00':return m,raw
 raise ValueError('missing SFNT')
def checksum(b):
 b+=bytes((-len(b))%4);return sum(struct.unpack('>'+str(len(b)//4)+'I',b))&0xffffffff
def patch(pdf,source):
 m,b=sfnt(pdf);_,e=sfnt(source);a=TTFont(io.BytesIO(b));e=TTFont(io.BytesIO(e));assert a.getGlyphOrder()==e.getGlyphOrder();b=bytearray(b);tables={}
 for i in range(struct.unpack_from('>H',b,4)[0]):
  pos=12+16*i;tag,cs,off,length=struct.unpack_from('>4sIII',b,pos);tables[tag]=(pos,off,length)
 changes=[];_,off,_=tables[b'hmtx']
 for i,n in enumerate(a.getGlyphOrder()):
  x,y=a['glyf'][n].getCoordinates(a['glyf']),e['glyf'][n].getCoordinates(e['glyf']);assert all(list(u)==list(v)for u,v in zip(x,y)),n
  w,l=a['hmtx'][n];ew,el=e['hmtx'][n];assert w==ew
  if l!=el:changes.append({'glyph':n,'native':l,'original':el})
  struct.pack_into('>h',b,off+4*i+2,el)
 pos,off,length=tables[b'hmtx'];struct.pack_into('>I',b,pos+4,checksum(bytes(b[off:off+length])));_,head,_=tables[b'head'];struct.pack_into('>I',b,head+8,0);struct.pack_into('>I',b,head+8,(0xb1b0afba-checksum(bytes(b)))&0xffffffff);assert checksum(bytes(b))==0xb1b0afba
 assert len(b)==len(m.group(1));return pdf[:m.start(1)]+b+pdf[m.end(1):],changes
def render(path,dpi):
 d=QPdfDocument();assert d.load(str(path))==QPdfDocument.Error.None_ and d.pageCount()==1;s=d.pagePointSize(0);raw=d.render(0,QSize(round(s.width()*dpi/72),round(s.height()*dpi/72)));out=QImage(raw.size(),QImage.Format.Format_RGB32);out.fill(0xffffffff);p=QPainter(out);p.drawImage(0,0,raw);p.end();out=out.convertToFormat(QImage.Format.Format_RGBA8888);return np.frombuffer(out.constBits(),np.uint8).copy().reshape(-1,4)
rows=[]
with tempfile.TemporaryDirectory(prefix='ingetrazo-bearing-') as td:
 for n in ['helvetica','helvetica-italic']:
  source=z.read('original-fonts/'+n+'.original.pdf');native=z.read('native-font-pages/'+n+'.native.pdf');fixed,changes=patch(native,source);paths=[]
  for stem,data in [('original',source),('native',native),('bearing',fixed)]:
   p=Path(td)/(stem+'.pdf');p.write_bytes(data);paths.append(p)
  for dpi in [72,150,300]:
   a,b,c=[render(p,dpi)for p in paths];rows.append({'case':n,'dpi':dpi,'original_changed_pixels':int(np.any(a!=b,axis=1).sum()),'bearing_only_changed_pixels':int(np.any(a!=c,axis=1).sum()),'bearing_changes':changes,'negative_original_native_rejected':bool(np.any(a!=b))})
r={'artifact_sha256':hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest(),'scope':'Only native PDF hmtx lsb and required SFNT checksums changed; coordinates/endpoints/flags/advances required identical; diagnostic not production proof','rows':rows,'all_bearing_only_exact':all(x['bearing_only_changed_pixels']==0 for x in rows)};Path(sys.argv[2]).write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
