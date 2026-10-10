#!/usr/bin/env python3
"""Literal paired image-plane pixels; strict equality and retained wrong shaders."""
import json,sys,copy
from pathlib import Path
root=Path(sys.argv[1]);native=Path(sys.argv[2]);old=Path(sys.argv[3]);wrong=Path(sys.argv[4])
s=json.loads((root/"original-alpha-results.json").read_text())["cases"]
def load(p):return {r["case"]:r for r in json.loads((p/"native-alpha-results.json").read_text())["cases"]}
device=json.loads((native/"native-alpha-results.json").read_text())["renderer_device"]
assert device["vendor_id"]==0x10de and device["device_type"]=="DISCRETE_GPU" and "NVIDIA GeForce RTX 5090" in device["name"] and device["software_fixture_requested"] is False,"wrong or software native GPU"
assert isinstance(device["device_id"],int) and device["device_id"]>0,"missing physical device identity"
n=load(native);o=load(old);w=load(wrong)
required={"image-"+v for v in ["quadrants","alpha","cutout","opacity-0999","opacity-below","reversed","wireframe","hidden-line","xray","hidden-layer","overlap","opaque-behind","faded-behind","threshold-behind","below-threshold-behind","coplanar-trace"]}
assert {r["case"]for r in s}==set(n)==set(o)==set(w)==required,"missing mandatory rendered family"
for r in s:
 assert "NVIDIA GeForce RTX 5090" in r["gl_info"]["renderer"],"wrong original GPU"
 assert r["samples"]==n[r["case"]]["samples"],r["case"]+" strict RGBA mismatch"
 assert n[r["case"]]["warm_pose_metrics"]=={"pose_updates":11,"geometry_uploads":0,"image_uploads":0,"hashes":0,"decodes":0},"warm poses caused admission or geometry work"
cut=next(r for r in s if r["case"]=="image-cutout");assert cut["samples"]!=o["image-cutout"]["samples"],"omitted Bayer negative accepted";assert cut["samples"]!=w["image-cutout"]["samples"],"wrong origin Bayer negative accepted"
for family in ["image-alpha","image-threshold-behind","image-below-threshold-behind","image-reversed"]:
 bad=copy.deepcopy(n);bad[family]["samples"][0][0]^=1
 assert any(r["samples"]!=bad[r["case"]]["samples"]for r in s),"pixel mutation accepted"
print(json.dumps({"cases":16,"samples":sum(len(r["samples"])for r in s),"exact":True,"retained_shader_failures":2,"pixel_mutations_rejected":4,"scope":"paired actual NVIDIA GL/Vulkan samples only; no whole-image/general image-tool closure"}))
