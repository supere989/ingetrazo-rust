#!/usr/bin/env python3
"""Strict retained software CAD2D controls; requires original QColor evidence."""
import argparse,json,re
from pathlib import Path
from PySide6.QtGui import QImage
p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('--output',type=Path);args=p.parse_args()
root=args.root;reference=json.loads((root/'original-reference.json').read_text());expected=reference['parent_case']['rgba'][0]
assert reference['rgba_input']==[37,93,157,128] and expected==[146,174,206,255]
rows=[]
def sample(image,x,y):return list(image.pixelColor(x,y).getRgb())
def check(image,point,value):assert sample(image,*point)==value,(point,sample(image,*point),value)
for label in ['baseline','callback-only','callback-error','pressure','uniform']:
 log=(root/label/'startup.log').read_text();metrics=re.search(r'CAD2D metrics: Stats \{([^}]+)',log);assert metrics,label
 stats={key:int(value) for key,value in re.findall(r'(\w+): (\d+)',metrics.group(1))}
 assert stats['uploads']==(3 if label=='pressure' else 1) and stats['cache_hits']>0 and stats['callbacks']==stats['draws']==stats['binds'],stats
 if label=='pressure':assert 'PASS CAD2D actual vertex allocation/index budget denial/prior drawable/rollback/retry' in log and stats['rolled_back_buffers']==1
 if label=='callback-error':assert log.count('EXPECTED callback error:')==1
 checks=[]
 for size in ['800x600','1100x700']:
  image=QImage(str(root/label/f'resize-{size}.png'));assert not image.isNull(),(label,size)
  if label=='callback-only':assert sample(image,60,60)[:3]!=sample(image,5,5)[:3]
  else:
   check(image,(35,35),[0,0,0,255]);check(image,(140,40),[255,0,0,255]);check(image,(180,40),[0,255,0,255]);check(image,(140,80),[0,0,255,255]);check(image,(180,80),[255,255,255,255])
   if label=='uniform':check(image,(80,60),[255,0,0,255]);check(image,(50,60),[255,255,255,255]);check(image,(105,60),[255,255,255,255])
   else:check(image,(60,60),expected)
  checks.append(size)
 rows.append({'case':label,'captures':checks,'stats':stats,'status':'PASS'})
image=QImage(str(root/'baseline'/'resize-800x600.png'));negative=[]
for bad in [[145,173,206,255],[37,93,157,128],[255,255,255,255]]:
 try:check(image,(60,60),bad)
 except AssertionError:negative.append({'rejected':bad})
 else:raise AssertionError('Negative control unexpectedly passed')
result={'status':'PASS','scope':'literal Qt interior color, native software inline ordering/nearest texture/restored UI/scissor/uniform cache/error and pressure lifecycle; not shape AA/hardware/Metal','cases':rows,'negative_controls':negative}
if args.output:args.output.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
