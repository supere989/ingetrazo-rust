#!/usr/bin/env python3
import sys,json
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core.style import Style
from core.sun import ShadowSettings
from acceptance_context import context
rows=[]
for kind,raw in [("style",{}),("style",{"name":17,"edge_color":[.1,.2,.3,.4],"front_color":[".7",True,.9]}),("style",{"name":None,"sky_color":"rgb","background":{"red":1,"green":2,"blue":3}}),("style",{"back_color":[.1,.2,.3,"invalid"]}),("style",{"back_color":[True,".2",.3,.4]}),("style",{"edges":[],"sky":{},"name":["Named"]}),("style",{"edge_color":None}),("shadows",{"day":"12.8","minute":True,"darkness":True}),("shadows",{"day":True,"month":"  +7  ","hour":13.9,"minute":"1_2","utc_offset":" -5 "}),("shadows",{"day":False,"darkness":False,"utc_offset":True}),("shadows",{"utc_offset":4294967296}),("shadows",{"utc_offset":"-5.5"})]:
 try:result=(Style if kind=="style"else ShadowSettings).from_dict(raw).to_dict();error=None
 except (TypeError,ValueError,OverflowError)as e:result=None;error=type(e).__name__
 rows.append({"kind":kind,"input":raw,"output":result,"error":error})
Path(sys.argv[1]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
