#!/usr/bin/env python3
"""Capture the pinned original VCB numeric grammar, without a render/startup."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
from types import SimpleNamespace
from contextlib import nullcontext
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
original=Path('ingetrazo').resolve();sys.path.insert(0,str(original))
from core import units
from views.viewport import Viewport
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==pin
out=Path(sys.argv[1]);out.mkdir(exist_ok=True,parents=True)
lengths=['2','30cm','1500mm','2m','2"','2in','2.5in',"1'",'1ft','1.5ft',"1'6\"","1'6","0'6\"",'3/4"','1/2in',"1'3/4\"",'-2"',"-1'",'.5in','1 1/2"','1-1/2"','-1 1/2"',"1'6 1/2\"","1'6-1/2\"","1' 1/2\"",'2,5','0','-0','1000000000','1000000001','1e3','+2','NaN','inf','','in','"','2xBAD','3/0"',"1''",'abc','2mm3','1-2','3/.5','2 m','1..0','--1','2IN','2\u00a0m']
lists=['200,100','3,2,1','2,5;1,2','200, 100','200;100','200 100','1 2','1 2 3','1 2 3 4','1 1/2";3 1/2"','3,2;1\'6";10cm','1m;2','2;3ft',';1;;2;','2,,3','1\t2','1-1/2;2 3/4','0;0;0']
rows=[]
for unit in units.UNIT_CHOICES:
 units.bind_scene(SimpleNamespace(units=dict(length=unit,precision=2)))
 for text in lengths:
  for unitless in [False,True]:
   with units.unitless_numbers()if unitless else nullcontext():value=Viewport._parse_value_buffer(text)
   if isinstance(value,(int,float)):value=[value]
   elif isinstance(value,tuple):value=list(value)
   assert value is None or all(isinstance(v,float)for v in value),(text,value)
   rows.append(dict(unit=unit,text=text,mode='fields',comma_lists=False,unitless=unitless,value=value))
 for text in lists:
  for comma in [False,True]:
   value=Viewport._parse_value_buffer(text,comma_lists=comma)
   if isinstance(value,(int,float)):value=[value]
   elif isinstance(value,tuple):value=list(value)
   rows.append(dict(unit=unit,text=text,mode='fields',comma_lists=comma,unitless=False,value=value))
 for text in ['45','45,5','3:12','-3:12','.5:2','1:0','1:-2','1 :2','3:12:4','2m','3/4','NaN']:
  with units.unitless_numbers():value=Viewport._parse_value_buffer(text)
  if isinstance(value,tuple)and value[0]=='ratio':value=value[1]
  if not isinstance(value,(int,float)):value=None
  rows.append(dict(unit=unit,text=text,mode='angle',comma_lists=False,unitless=True,value=value))
report=dict(source_pin=pin,source_files={name:hashlib.sha256((original/name).read_bytes()).hexdigest()for name in ['views/viewport.py','core/units.py']},rows=rows)
(out/'original-measurements.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(dict(rows=len(rows),source_pin=pin)))
