#!/usr/bin/env python3
"""Strict retained production activation checks, including raw archive integrity."""
import hashlib,io,json,zipfile
from pathlib import Path
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[1]/"docs/audits/annotation-stencil-production"
m=json.loads((root/"archive.json").read_text());raw=(root/"raw-evidence.zip").read_bytes()
assert hashlib.sha256(raw).hexdigest()==m["zip_sha256"]
z=zipfile.ZipFile(io.BytesIO(raw));assert set(z.namelist())==set(m["files"])
for name,digest in m["files"].items():assert hashlib.sha256(z.read(name)).hexdigest()==digest,name
def image(name):
 a=np.asarray(Image.open(io.BytesIO(z.read(name))).convert("RGBA"));assert a.shape==(600,800,4),name;return a
def exact(a,b):assert np.array_equal(a,b),"Strict whole-image mismatch"
for phase in ("annotations-on","annotations-off","annotations-restored"):
 exact(image(f"application-parity/original/{phase}.png"),image(f"application-parity/native/{phase}.png"))
assert np.any(image("application-parity/native/annotations-on.png")!=image("application-parity/native/annotations-off.png"))
exact(image("application-parity/native/annotations-on.png"),image("application-parity/native/annotations-restored.png"))
names=[f"triangle-alpha-{a}" for a in (0,1,127,128,230,255)]+["triangle-overlap","triangle-sliver"]
for name in names:exact(image(f"triangles/original/{name}.png"),image(f"triangles/native/{name}/native.png"))
for order in ("timing-first","timing-reverse"):
 for mode in ("batched","unbatched"):
  exact(image("additional/ellipse-alpha.png"),image(f"{order}/{mode}/ellipse-alpha/native.png"))
  states=json.loads(z.read(f"{order}/{mode}/ellipse-alpha/benchmark-states.json"));assert len(states)==2
  for key in ("geometry_uploads","query_uploads","cached_bytes","committed_bytes"):assert states[0][key]==states[1][key]
  assert states[1]["submitted_serial"]>states[0]["submitted_serial"]
  log=z.read(f"{order}/{mode}/ellipse-alpha/runtime.log").decode();assert "Native annotation GPU timing:" in log
result=json.loads(z.read("lifecycle/application/result.json"));assert len(result["rows"])==6 and result["passed"]
assert [r["name"] for r in result["rows"]]==["add","delete","add","refuse","retry","delete"]
assert result["rows"][3]["state"]["held_refusal"]
def app_image(name):
 a=np.asarray(Image.open(io.BytesIO(z.read("lifecycle/application/"+name+".png"))).convert("RGBA"));assert a.shape==(700,1000,4);return a
exact(app_image("baseline"),app_image("2-delete"));exact(app_image("3-add"),app_image("4-refuse"))
assert np.any(app_image("baseline")!=app_image("1-add"))
assert "committed_bytes=0" in z.read("lifecycle/application/runtime.log").decode()
for kind in ("app","xvfb"):
 cleanup=json.loads(z.read("lifecycle/application/"+kind+"-cleanup.json"));assert cleanup["returncode"]==0 and not cleanup["terminate_timeout"]
assert b"PASS actual app add/delete/held frame/managed texture retry" in z.read("lifecycle/application.log")
# Literal retained wrong-style output must fail the same strict pixel comparator.
try:exact(image("failed-style-alpha/original/annotations-on.png"),image("failed-style-alpha/native/annotations-on.png"))
except AssertionError:pass
else:raise AssertionError("Retained wrong-alpha negative unexpectedly accepted")
try:exact(image("application-parity/native/annotations-on.png"),image("application-parity/native/annotations-off.png"))
except AssertionError:pass
else:raise AssertionError("Missing annotation negative unexpectedly accepted")
print("PASS 3 production viewport + 8 translucent triangles + 4 timing images, lifecycle/warm controls, archive and two literal negatives")
