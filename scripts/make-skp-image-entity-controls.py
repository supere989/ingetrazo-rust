#!/usr/bin/env python3
"""Pinned writer MFC Image records; writer warns schema lacks SketchUp calibration."""
import sys,hashlib,json,subprocess
from pathlib import Path
oracle=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()=='291700bc213655a00461838de71e5f206311e619'
sys.path.insert(0,str(oracle/'packages/python/src'))
from openskp.create import create
from PySide6.QtGui import QImage,QColor
rows=[]
for name,alpha in [('opaque',255),('cutout',0),('translucent',64)]:
 image=QImage(2,2,QImage.Format_RGBA8888);image.fill(QColor(210,30,80,255));image.setPixelColor(0,0,QColor(30,210,80,alpha));image.save(str(out/(name+'.png')))
 b=create();b.add_image(str(out/(name+'.png')),20,10,translation=(5.,-2.,3.),matrix3x3=(0.,-2.,0.,3.,0.,0.,0.,0.,1.));b.add_face([(0,0,0),(2,0,0),(2,2,0),(0,2,0)]);b.save(out/(name+'.skp'));rows.append({'name':name,'sha256':hashlib.sha256((out/(name+'.skp')).read_bytes()).hexdigest(),'billboard':name=='cutout'})
(out/'manifest.json').write_text(json.dumps({'parser_pin':'291700bc213655a00461838de71e5f206311e619','schema_scope':'pinned writer CImage; SketchUp calibration explicitly unverified','cases':rows},indent=2)+'\n')
