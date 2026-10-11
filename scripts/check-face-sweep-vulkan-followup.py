#!/usr/bin/env python3
"""Read-only strict whole-frame replay; --verify-recorded authenticates a FAIL."""
import argparse,hashlib,io,json,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument("audit");parser.add_argument("source_audit");parser.add_argument("--verify-recorded",action="store_true");args=parser.parse_args()
def archive(path):
 path=Path(path);manifest=json.loads((path/"manifest.json").read_text());raw=(path/"records.zip").read_bytes();assert hashlib.sha256(raw).hexdigest()==manifest["archive_sha256"]
 z=zipfile.ZipFile(io.BytesIO(raw));assert len(z.namelist())==len(set(z.namelist()))==len(manifest["records"])
 for r in manifest["records"]:
  b=z.read(r["path"]);assert len(b)==r["bytes"] and hashlib.sha256(b).hexdigest()==r["sha256"]
 return z,manifest
def image(raw,extent):
 with Image.open(io.BytesIO(raw)) as im:
  assert im.size==extent;return np.asarray(im.convert("RGBA")).copy()
def compare(a,b):
 assert a.shape==b.shape
 delta=np.abs(a.astype(np.int16)-b.astype(np.int16));return int(np.any(delta!=0,axis=2).sum()),int(delta.max())
z,manifest=archive(args.audit);source,source_manifest=archive(args.source_audit)
assert source_manifest["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
recipes=json.loads(source.read("complete-config/original.json"))["cases"]
assert len(recipes)==6 and len({r["name"]for r in recipes})==6
rows=[];negatives=0
for recipe in recipes:
 name=recipe["name"];assert len(recipe["frames"])==4
 for row in recipe["frames"]:
  phase=row["phase"];raw=source.read("complete-config/"+row["file"]);assert hashlib.sha256(raw).hexdigest()==row["sha256"]
  prefix="source-zero-phase" if phase in ("base","cancel") else "source-first-protocol"
  file={"base":"1-acquire.png","preview":"2-drag.png","offset":"3-drag.png","cancel":"4-drag.png"}[phase]
  a=image(raw,(800,600));b=image(z.read(prefix+"/"+name+"/"+file),(800,600));different,maximum=compare(a,b)
  expected=(539,81) if name=="reversed-door" and phase=="preview" else (1079,122) if name=="reversed-door" and phase=="offset" else (0,0)
  assert (different,maximum)==expected,"Frozen result changed; do not fit expectations"
  rows.append(dict(case=name,phase=phase,different_pixels=different,maximum=maximum,exact=different==0))
  changed=a.copy();changed[0,0,0]^=1;assert compare(a,changed)==(1,1);negatives+=1
  if phase in ("base","cancel"):
   omitted=image(source.read("complete-config/"+row["gl_only_file"]),(800,600));assert compare(a,omitted)[0]>0;negatives+=1
  transport=json.loads(z.read(prefix+"/"+name+"/transport.json"));assert transport["returncode"]==0 and transport["compiled_source"]=='c6be7538880c0451da222f2a78efd756a9da8abe'
# Actual application controls remain distinct from the viewport-only source observer.
app=json.loads(z.read("transaction-final/result.json"));assert app["passed"] and app["source_sha"]=='c6be7538880c0451da222f2a78efd756a9da8abe'
names=[r["name"]for r in app["rows"]];assert names==["acquire","drag","drag","refuse","retry","typed","refuse-font","retry","refuse-cad","retry","cancel","acquire","drag","commit","undo","save"]
for before_index,refused_index,retry_index in [(2,3,4),(5,6,7),(7,8,9)]:
 before=app["rows"][before_index];refused=app["rows"][refused_index];retry=app["rows"][retry_index]
 assert before["state"]["committed_bytes"]==refused["state"]["committed_bytes"] and before["state"]["world"]==refused["state"]["world"]
 assert refused["state"]["prepare_error"] and not retry["state"]["prepare_error"]
 a=image(z.read("transaction-final/"+str(before_index+1)+"-"+before["name"]+".png"),(1000,700));b=image(z.read("transaction-final/"+str(refused_index+1)+"-"+refused["name"]+".png"),(1000,700));assert compare(a,b)==(0,0)
 if refused_index in (6,8):assert refused["state"]["old_label_owner_alive"] is True and retry["state"]["old_label_owner_alive"] is False
assert "committed_bytes=0" in z.read("transaction-final/runtime.log").decode()
assert app["rows"][-1]["state"]["dirty"] is False
assert len(rows)==24 and sum(r["exact"]for r in rows)==22
result=dict(frozen_verified=True,source_parity=False,exact_frames=22,total_frames=24,negative_controls=negatives,application_controls=True,rows=rows)
print(json.dumps(result,indent=2))
raise SystemExit(0 if args.verify_recorded else 1)
