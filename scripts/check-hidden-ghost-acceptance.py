#!/usr/bin/env python3
"""Read-only strict archived ghost frame + application acceptance."""
import argparse,hashlib,json,io,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
p=argparse.ArgumentParser();p.add_argument("audit");args=p.parse_args()
root=Path(args.audit);manifest=json.loads((root/"manifest.json").read_text())
archive=root/manifest["archive"];assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest["archive_sha256"]
with zipfile.ZipFile(archive) as z:
 def read(name):return z.read(name)
 assert set(z.namelist())==set(manifest["files"])
 for name,digest in manifest["files"].items():assert hashlib.sha256(read(name)).hexdigest()==digest,name
 original=json.loads(read("original/original-alpha-results.json"));native=json.loads(read("native/native-alpha-results.json"))
 names=[kind+"-"+state for kind in ["face","edge","object","textured-object","nested-object","occluded-face"] for state in ["off","on"]]
 assert [r["case"] for r in original["cases"]]==names
 assert [r["case"] for r in native["cases"]]==names
 assert manifest["original_commit"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
 def image(name):
  im=Image.open(io.BytesIO(read(name))).convert("RGBA");assert im.size==(800,600);return np.array(im)
 def compare(a,b):assert a.shape==b.shape and np.array_equal(a,b),"Whole RGBA difference"
 for row in original["cases"]:
  name=row["case"];assert row["gl_info"]["vendor"]=="NVIDIA Corporation" and "RTX 5090" in row["gl_info"]["renderer"]
  assert row["ghost_samples"]==4
  assert hashlib.sha256(read("original/"+name+".igz")).hexdigest()==row["native_input_sha256"]
  compare(image("original/"+name+".png"),image("native/"+name+".png"))
 for kind in names[::2]:
  name=kind.removesuffix("-off")
  changed=np.any(image("original/"+name+"-on.png")!=image("original/"+name+"-off.png"),2).sum()
  assert (changed==0) if name=="occluded-face" else (changed>0)
 try:compare(image("original/face-on.png"),image("native/face-off.png"))
 except AssertionError:pass
 else:raise AssertionError("Missing ghost negative accepted")
 try:read("native/missing-on.png")
 except KeyError:pass
 else:raise AssertionError("Missing case negative accepted")
 app=json.loads(read("application/result.json"));assert app["passed"] is True
 actions=[r["name"] for r in app["rows"]]
 assert actions==manifest["application_actions"] and len(actions)>=17
 for row in app["rows"]:
  assert hashlib.sha256(read("application/"+str(row["state"]["nonce"])+"-"+row["name"]+".png")).hexdigest()==row["image_sha256"]
 assert "committed_bytes=0" in read("application/runtime.log").decode()
 print("PASS 12 exact whole RGBA frames, positive/occlusion controls, archived actual app workflow and two negatives")
