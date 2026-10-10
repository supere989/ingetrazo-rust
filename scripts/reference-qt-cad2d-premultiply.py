#!/usr/bin/env python3
"""Actual QColor/QPainter solid-brush raster across all byte channel/alpha pairs."""
import argparse,hashlib,json
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtCore import qVersion
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
image=QImage(256,256,QImage.Format_ARGB32_Premultiplied);image.fill(QColor(0,0,0,255));paint=QPainter(image)
for alpha in range(256):
 for channel in range(256):paint.fillRect(channel,alpha,1,1,QColor(channel,channel,channel,alpha))
paint.end();data=bytes((image.pixel(c,a)>>16)&255 for a in range(256) for c in range(256));(a.output/'channel-alpha.bin').write_bytes(data);image.save(str(a.output/'channel-alpha.png'))
# Keep the tempting QRgb32 shortcut's negative evidence, rather than replacing
# the actual paint protocol with a library helper assumed equivalent.
wrong=[]
for alpha in range(256):
 for channel in range(256):
  n=channel*alpha;shortcut=(n+(n>>8)+128)>>8;actual=data[alpha*256+channel]
  if shortcut!=actual:wrong.append({'channel':channel,'alpha':alpha,'actual':actual,'qrgb32_shortcut':shortcut})
result={'source':'Actual QPainter.fillRect QColor on ARGB32_Premultiplied opaque black; every channel/alpha0..255 pair','qt_version':qVersion(),'pairs':65536,'sha256':hashlib.sha256(data).hexdigest(),'qrgb32_negative_mismatches':wrong,'qt_source':'https://github.com/qt/qtbase/blob/6.12/src/gui/painting/qrgba64.h'}
(a.output/'original.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
