#!/usr/bin/env python3
"""Independent screenshot pixel acceptance for actual Vulkan nearest/partial texture uploads."""
import argparse,json
from pathlib import Path
from PySide6.QtGui import QImage,QColor
a=argparse.ArgumentParser();a.add_argument("capture_directory");args=a.parse_args();root=Path(args.capture_directory)
rows=[];images={}
for filename,topright in [("startup.png",(0,255,0)),("partial-texture.png",(255,255,0))]:
 im=QImage(str(root/filename));assert not im.isNull(),f"Missing rendered capture {filename}";images[filename]=im
 expected={(30,120):(255,0,0),(90,120):topright,(30,180):(0,0,255),(90,180):(255,255,255),(45,135):(255,0,0),(75,135):topright,(45,165):(0,0,255),(75,165):(255,255,255)}
 for xy,rgb in expected.items():
  actual=im.pixelColor(*xy).getRgb()[:3];rows.append({"image":filename,"pixel":list(xy),"expected_rgb":list(rgb),"actual_rgb":list(actual),"match":tuple(actual)==rgb})
lost_texel=images["partial-texture.png"].copy();lost_texel.setPixelColor(30,180,QColor(255,255,255))
controls=[{"case":"lost-partial-update","detected":images["startup.png"].pixelColor(90,120).getRgb()[:3]!=(255,255,0)},{"case":"lost-untouched-texel","detected":lost_texel.pixelColor(30,180).getRgb()[:3]!=(0,0,255)}]
report={"passed":all(r["match"]for r in rows),"negative_controls":controls,"samples":rows,"scope":"Actual ash Vulkan UI nearest magnification and1pixel partial upload,3unchangedtexels retained, initial/updated80pximage. LinuxXvfbllvmpipe; minification,wrap,compressedmips,CAD3Dtextures,Metalhardware notproved."}
(root/"texture-evidence.json").write_text(json.dumps(report,indent=2)+"\n")
assert report["passed"] and all(c["detected"]for c in controls);print("PASS nearest/partial Vulkan texture16samples+2controls")
