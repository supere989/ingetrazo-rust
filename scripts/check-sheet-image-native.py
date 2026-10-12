#!/usr/bin/env python3
"""Strict retained physical source images versus actual native output; no tolerance."""
import json,sys,hashlib,struct
from pathlib import Path
from PySide6.QtGui import QImage
class Image:
 @staticmethod
 def open(path):
  image=QImage(str(path));assert not image.isNull();return Image(image.convertToFormat(QImage.Format_RGB888))
 def __init__(self,image):self.image=image;self.size=(image.width(),image.height())
 def convert(self,mode):assert mode=="RGB";return self
 def tobytes(self):
  image=self.image;raw=bytes(image.constBits());return b"".join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
NAMES=['blank-portrait','blank-landscape','blank-a3','shapes','shapes-jpeg','shapes-portrait','text']
def physical(path):
 blob=path.read_bytes()
 if blob.startswith(b'\x89PNG\r\n\x1a\n'):
  at=8;out={}
  while at<len(blob):
   length=struct.unpack('>I',blob[at:at+4])[0];kind=blob[at+4:at+8];payload=blob[at+8:at+8+length]
   if kind==b'pHYs':out['density']=list(struct.unpack('>IIB',payload))
   if kind==b'IHDR':out['header']=list(struct.unpack('>IIBBBBB',payload))
   at+=length+12
  return out
 at=2
 while at<len(blob)and blob[at]==255:
  tag=blob[at+1];length=struct.unpack('>H',blob[at+2:at+4])[0];payload=blob[at+4:at+2+length]
  if tag==224 and payload.startswith(b'JFIF\0'):return {'jfif':{'unit':payload[7],'density':list(struct.unpack('>HH',payload[8:12]))}}
  if tag==218:break
  at+=length+2
 raise AssertionError('Missing physical image metadata')
def check(source,native,out,views=None):
 original=json.loads((source/'source.json').read_text());record=json.loads((native/'native.json').read_text())
 inputs=[(source,row)for row in original['images']]
 expected_names=NAMES.copy()
 if views is not None:
  addendum=json.loads((views/'source.json').read_text());names=['view-current','view-saved','view-cropped','view-hidden-thin'];assert [r['case']for r in addendum['images']]==names;expected_names+=names;inputs.extend((views,row)for row in addendum['images'])
 assert [v['case']for v in original['images']]==NAMES
 assert [v['case']for v in record['images']]==expected_names
 comparisons=[]
 for (input,expected),actual in zip(inputs,record['images']):
  name=expected['case'];file=expected['image']['file'];assert actual['file']==file
  assert type(actual['dpi_remembered'])is int and actual['dpi_remembered']==expected['dpi']
  assert actual['metrics']['dpi']==expected['dpi']
  source_metadata=physical(input/file);native_metadata=physical(native/file);assert source_metadata==native_metadata,(name,source_metadata,native_metadata)
  a=Image.open(input/file).convert('RGB');b=Image.open(native/file).convert('RGB');assert a.size==b.size==tuple(actual['pixels'])
  ar=a.tobytes();br=b.tobytes();assert hashlib.sha256(ar).hexdigest()==expected['image']['rgb_sha256'];diff=sum(ar[i:i+3]!=br[i:i+3]for i in range(0,len(ar),3));maximum=max((abs(x-y)for x,y in zip(ar,br)),default=0)
  comparisons.append({'case':name,'different_pixels':diff,'max_channel_error':maximum,'source_rgb_sha256':hashlib.sha256(ar).hexdigest(),'native_rgb_sha256':hashlib.sha256(br).hexdigest(),'pixels':list(a.size),'exact':diff==0})
 assert len(record['controls'])==len(original['controls'])==6
 for expected,actual in zip(original['controls'],record['controls']):
  for key in ['case','refreshes','dialog_args','settings_before','settings_after','error']:assert actual[key]==expected[key],(key,expected,actual)
  assert [r['file']for r in actual['images']]==[r['file']for r in expected['images']]
  for image in expected['images']:
   file=image['file'];assert physical(source/file)==physical(native/file)
   a=Image.open(source/file).convert('RGB');b=Image.open(native/file).convert('RGB');assert a.size==b.size and a.tobytes()==b.tobytes()
 assert all(record[k] is True for k in ['lazy_readback_refused','unsupported_preserved','stale_target_refused','modal_input_owned','no_temps'])
 report={'comparisons':comparisons,'exact_images':sum(v['exact']for v in comparisons),'images':len(expected_names),'scope':'Strict actual physical image bytes; JPEG encoder mismatch remains a failure, not tolerated. No full painter/desktop claim.'}
 out.write_text(json.dumps(report,indent=2)+'\n');return report
if __name__=='__main__':
 report=check(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]),Path(sys.argv[4])if len(sys.argv)>4 else None);print(json.dumps(report));sys.exit(0 if report['exact_images']==report['images']else 1)
