#!/usr/bin/env python3
"""Strict paired actual NVIDIA original OpenGL/native Vulkan29 border corpus."""
import json,sys,hashlib
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage
BASE="quadrants alpha cutout opacity-0999 opacity-below reversed wireframe hidden-line xray hidden-layer overlap opaque-behind faded-behind threshold-behind below-threshold-behind coplanar-trace oblique perspective uv-asymmetric uv-asymmetric-perspective uv-negative-v offscreen-clip near-clipped tinted-alpha selected selected-oblique selected-perspective selected-offscreen selected-near-clipped".split()
REQUIRED={"image-"+v for v in BASE}
def indexed(rows):
 assert len(rows)==len(REQUIRED),"Mandatory29 cases missing/extra"
 assert {r["case"]for r in rows}==REQUIRED,"Mandatory literal registry differs"
 return {r["case"]:r for r in rows}
def pixels(p):
 im=QImage(str(p)).convertToFormat(QImage.Format_RGBA8888)
 assert not im.isNull() and im.width()==800 and im.height()==600,str(p)
 return np.frombuffer(im.constBits(),dtype=np.uint8).reshape(600,800,4).copy()
def check(source,native):
 src=indexed(json.loads((source/"original-alpha-results.json").read_text())["cases"])
 report=json.loads((native/"native-ui-results.json").read_text());assert report["passed"]is True
 dst=indexed(report["cases"]);rows=[]
 for name in sorted(REQUIRED):
  a,b=src[name],dst[name];g=a["gl_info"];assert g["software"]is False and "NVIDIA"in g["vendor"] and "RTX 5090"in g["renderer"]
  assert not a["image_outline_ablation"] and not a["uv_mirror_ablation"] and a["projection_y_ablation"]is None
  assert a["outline_paint_metadata"]=={"paint_engine":13,"antialias":True}
  assert hashlib.sha256((source/(name+".igz")).read_bytes()).hexdigest()==a["native_input_sha256"]
  d=b["renderer_device"];assert d["vendor_id"]==0x10de and d["device_type"]=="DISCRETE_GPU" and "RTX 5090"in d["name"]
  assert b["captured"]is True;log=(native/b["backend_log"]).read_text();assert "Native bounded GPU cleanup complete; present_fences_enabled=true; committed_bytes=0"in log
  x,y=pixels(source/(name+".png")),pixels(native/(name+".png"));n=int(np.any(x!=y,axis=2).sum());assert n==0,(name,n)
  rows.append({"case":name,"pixels":480000,"differing_pixels":n,"renderer_device":d})
 return {"passed":True,"cases":rows,"total_pixels":13920000,"scope":"Mandatory29 paired actual NVIDIA OpenGL2/Vulkan full RGBA borders and surfaces; Metal/performance generalization open"}
if __name__=="__main__":print(json.dumps(check(*map(Path,sys.argv[1:3])),indent=2))
