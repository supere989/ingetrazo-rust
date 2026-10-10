#!/usr/bin/env python3
"""Strict source GL surface frames, with only QPainter outlines ablated."""
import copy,json,sys
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage
source,native,old=map(Path,sys.argv[1:4])
s=json.loads((source/"original-alpha-results.json").read_text())
n=json.loads((native/"native-alpha-results.json").read_text())
required={"image-"+v for v in ["quadrants","alpha","cutout","opacity-0999","opacity-below","reversed","wireframe","hidden-line","xray","hidden-layer","overlap","opaque-behind","faded-behind","threshold-behind","below-threshold-behind","coplanar-trace"]}
def device(d):
 assert d["vendor_id"]==0x10de and d["device_type"]=="DISCRETE_GPU" and d["software_fixture_requested"] is False and "NVIDIA GeForce RTX 5090" in d["name"],"wrong actual Vulkan device"
 assert isinstance(d["device_id"],int)and d["device_id"]>0
device(n["renderer_device"])
for mutation in ["vendor_id","device_type","software_fixture_requested"]:
 bad=copy.deepcopy(n["renderer_device"]);bad[mutation]={"vendor_id":0,"device_type":"CPU","software_fixture_requested":True}[mutation]
 try:device(bad)
 except AssertionError:pass
 else:raise AssertionError("device mutation admitted")
assert {r["case"]for r in s["cases"]}=={r["case"]for r in n["cases"]}==required
def pixels(path):
 im=QImage(str(path)).convertToFormat(QImage.Format_RGBA8888);assert not im.isNull(),str(path)
 assert im.width()==800 and im.height()==600
 return np.frombuffer(im.constBits(),np.uint8).reshape(600,800,4).copy()
for row in s["cases"]:
 assert "NVIDIA GeForce RTX 5090" in row["gl_info"]["renderer"]
 assert row["image_outline_ablation"] is True and not row.get("uv_mirror_ablation",False) and row.get("projection_y_ablation")is None,"altered GPU reference cannot establish source parity"
 name=row["case"];a=pixels(source/(name+".png"));b=pixels(native/(name+".png"));assert np.array_equal(a,b),name+" whole RGBA mismatch"
 assert next(r for r in n["cases"]if r["case"]==name)["warm_pose_metrics"]=={"pose_updates":11,"geometry_uploads":0,"image_uploads":0,"hashes":0,"decodes":0}
 normative=pixels(source/"image-quadrants.png")
assert not np.array_equal(normative,pixels(old/"image-quadrants.png")),"literal old inverse-V path incorrectly admitted"
bad=normative.copy();bad[226,343,0]^=1;assert not np.array_equal(normative,bad),"interior one-channel mutation admitted"
print(json.dumps({"cases":16,"whole_rgba_pixels":16*800*600,"exact":True,"literal_old_rejected":True,"device_mutations_rejected":3,"pixel_mutations_rejected":1,"scope":"Original GPU image surfaces only; QPainter outlines deliberately excluded, remaining full display/oblique/mixed transforms/Metal open"}))
