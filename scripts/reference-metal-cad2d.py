#!/usr/bin/env python3
"""Pinned actual QPainter source wholeRGBA reference; never production dispatch."""
import json,sys,hashlib
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtCore import QRectF,qVersion
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
rows=[]
for alpha in [0,1,127,128,254,255]:
 name=f'alpha-{alpha}'
 image=QImage(64,64,QImage.Format.Format_RGB32);image.fill(QColor(255,255,255))
 p=QPainter(image);p.fillRect(QRectF(8,8,48,48),QColor(37,93,157,alpha));p.fillRect(QRectF(16,16,32,32),QColor(1,7,19,128));p.end()
 rgba=image.convertToFormat(QImage.Format.Format_RGBA8888);data=bytes(rgba.constBits());(out/(name+'.rgba')).write_bytes(data);image.save(str(out/(name+'.png')))
 rows.append({'name':name,'alpha':alpha,'sha256':hashlib.sha256(data).hexdigest()})
(out/'reference.json').write_text(json.dumps({'qt':qVersion(),'target':'RGB32','size':[64,64],'cases':rows,'scope':'integer-aligned filled rectangles/source color-overlap; no arbitrary shape AA'},indent=2)+'\n')
print(json.dumps({'actual_qt':qVersion(),'cases':len(rows)}))
