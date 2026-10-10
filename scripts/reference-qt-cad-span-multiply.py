#!/usr/bin/env python3
"""Public QPainter DestinationIn validates Qt BYTE_MUL all256×256 pairs."""
import json,hashlib,struct
from PySide6.QtGui import QImage,QPainter
source=QImage(256,256,QImage.Format.Format_ARGB32_Premultiplied);mask=QImage(256,256,QImage.Format.Format_ARGB32_Premultiplied)
for channel in range(256):
 for coverage in range(256):
  source.setPixel(coverage,channel,0xff000000|(channel<<16)|((255-channel)<<8)|channel)
  mask.setPixel(coverage,channel,coverage<<24)
p=QPainter(source);p.setCompositionMode(QPainter.CompositionMode.CompositionMode_DestinationIn);p.drawImage(0,0,mask);p.end()
def mul(x,a):t=x*a;return (t+(t>>8)+128)>>8
raw=bytearray()
for channel in range(256):
 for coverage in range(256):
  expected=(coverage<<24)|(mul(channel,coverage)<<16)|(mul(255-channel,coverage)<<8)|mul(channel,coverage)
  actual=source.pixel(coverage,channel);assert actual==expected,(channel,coverage,hex(actual),hex(expected));raw.extend(struct.pack('<I',actual))
print(json.dumps({'passed':True,'pairs':65536,'channels_checked':262144,'sha256':hashlib.sha256(raw).hexdigest(),'law':'t=channel*coverage; (t+(t>>8)+128)>>8','scope':'Actual QPainter premultiplied DestinationIn, same source BYTE_MUL used by solid SourceOver span multiplication. Not brush global opacity or gradient interpolation.'},indent=2))
