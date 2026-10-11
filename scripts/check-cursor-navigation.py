#!/usr/bin/env python3
"""Strict source-handler/camera checker. No numeric tolerances."""
import json,sys,copy
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
def compare(n):
 assert s["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
 assert len(s["core"])==7 and len(n["core"])==7
 for i,(a,b) in enumerate(zip(s["core"],n["core"])):assert a["result"]==b,("camera",i,a["result"],b)
 names=["face-orbit","bounds-orbit","empty-orbit","depth-pan","wheel-cache","wheel-empty"]
 assert [x["name"] for x in s["sessions"]]==names
 assert [x["name"] for x in n["sessions"]]==names
 for a,b in zip(s["sessions"],n["sessions"]):
  assert a["extent"]==[800,600]
  assert len(a["rows"])==len(b["rows"])
  for i,(x,y) in enumerate(zip(a["rows"],b["rows"])):assert x==y,(a["name"],i,x,y)
compare(n)
negatives=0
for kind in ["target","yaw","distance","calls","pivot","inventory"]:
 bad=copy.deepcopy(n)
 if kind=="target":bad["core"][0]["target"][0]+=2**-20
 elif kind=="yaw":bad["core"][0]["yaw"]+=2**-40
 elif kind=="distance":bad["core"][2]["distance"]+=2**-30
 elif kind=="calls":bad["sessions"][0]["rows"][0]["acquisitions"]+=1
 elif kind=="pivot":bad["sessions"][0]["rows"][0]["pivot"][0]+=2**-20
 else:bad["sessions"].pop()
 try:compare(bad)
 except AssertionError:negatives+=1
 else:raise AssertionError("negative accepted "+kind)
print(json.dumps(dict(core_exact=7,source_sessions_exact=6,event_states_exact=sum(len(x["rows"]) for x in s["sessions"]),strict_negatives_rejected=negatives)))
