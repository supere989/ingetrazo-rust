#!/usr/bin/env python3
"""Independent exact source/native checks, plus deliberately corrupted controls."""
import copy,json,sys
from pathlib import Path
FIELDS=("name","face","edge","group","selection","visible_faces")
def check(a,b):
 assert len(a["rows"])==len(b["rows"])==56
 for x,y in zip(a["rows"],b["rows"]):
  for k in FIELDS:assert x[k]==y[k],(x["name"],k,x[k],y[k])
 assert b["consumer"]==dict.fromkeys(["locked_select","locked_pushpull","unlock_commit","undo_redo","cancel"],True)
def main():
 root=Path(sys.argv[1]);a=json.loads((root/"original.json").read_text());b=json.loads((root/"native.json").read_text());check(a,b)
 controls=[(1,"face",0),(29,"edge",0),(37,"group",0),(1,"selection",["face"]),(1,"visible_faces",[False]),(33,"group",0)]
 for i,key,value in controls:
  c=copy.deepcopy(b);assert c["rows"][i][key]!=value;c["rows"][i][key]=value
  try:check(a,c)
  except AssertionError:continue
  raise AssertionError("corrupted output accepted")
 print("PASS: 56 exact source picker/panel states; 5 dispatch/history checks; 6 corruptions rejected")
if __name__=="__main__":main()
