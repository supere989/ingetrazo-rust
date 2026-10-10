#!/usr/bin/env python3
"""Compare actual production native UI image composition with original Qt pixels."""
import json,re,subprocess,sys
from pathlib import Path
original=json.loads(Path(sys.argv[1]).read_text())['cases'];root=Path(sys.argv[2]);rows=[]
for o in original:
 name=o['case'];folder='alpha-ui-black' if name=='red-half-black' else 'alpha-ui-'+name
 image=root/folder/'resize-800x600.png'
 text=subprocess.check_output(['convert',str(image),'-crop','1x1+400+300','-format','%[fx:round(255*r)],%[fx:round(255*g)],%[fx:round(255*b)]','info:'],text=True)
 pixel=list(map(int,text.split(',')))
 assert pixel==o['center_rgba'][:3],(name,pixel,o['center_rgba'])
 rows.append({'case':name,'pixel':pixel,'image':str(image)})
print(json.dumps({'cases':rows,'scope':'Actual native Vulkan UI compositor original alpha center controls; not backpaint/texture/general parity'},indent=2))
