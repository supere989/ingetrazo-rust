#!/usr/bin/env python3
"""Strict QPainter/direct CAD2D full constant-interior comparison. Fail retained differences."""
import argparse,json
from pathlib import Path
from PySide6.QtGui import QImage,QPainter,QColor
p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('--retain-failure',action='store_true');a=p.parse_args()
cases=[('alpha0',[17,83,211,0]),('alpha1',[17,83,211,1]),('alpha127',[17,83,211,127]),('alpha128',[17,83,211,128]),('alpha254',[17,83,211,254]),('alpha255',[17,83,211,255]),('dark',[1,7,19,128]),('light',[251,243,229,128]),('unequal',[5,127,250,73]),('terrain-override',[37,93,157,64])]
rows=[]
for name,rgba in cases:
 im=QImage(80,80,QImage.Format.Format_ARGB32);im.fill(QColor('white'));painter=QPainter(im);painter.fillRect(0,0,80,80,QColor(*rgba));painter.end();expected=list(im.pixelColor(40,40).getRgb());samples=[]
 directory=a.root/(name+'-sequential');life=json.loads((directory/'lifecycle.json').read_text());assert life['passed'] and life['clean_close'] and life['owned_client_only']
 log=(directory/'startup.log').read_text();assert 'NVIDIA GeForce RTX 5090' in log and 'llvmpipe' not in log
 for size in ['800x600','1100x700']:
  image=QImage(str(directory/('resize-'+size+'.png')));assert not image.isNull() and (image.width(),image.height())==tuple(map(int,size.split('x')))
  values={tuple(image.pixelColor(x,y).getRgb()) for x in range(45,95) for y in range(45,95)};samples.append({'size':size,'actual_rgba':list(map(list,sorted(values))),'pixels':2500,'exact':values=={tuple(expected)}})
 rows.append({'name':name,'rgba':rgba,'expected':expected,'native':samples,'passed':all(v['exact'] for v in samples)})
result={'passed':all(r['passed'] for r in rows),'cases':rows,'scope':'Constant 50x50 interiors at two sizes; terrain label is direct explicit alpha only, no terrain routing/AA/Metal closure.'}
(a.root/'verdict.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if not result['passed'] and not a.retain_failure:raise SystemExit(1)
