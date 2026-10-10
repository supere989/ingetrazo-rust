#!/usr/bin/env python3
"""Actual Qt JPEG decoder versus native pixels; strict failure retained."""
import ast, hashlib, json, subprocess, sys, zipfile
from pathlib import Path
from PySide6.QtCore import Qt, qVersion
from PySide6.QtGui import QImage
assert qVersion() == "6.12.0"
igz, native_json, native_raw, kernel, output = map(Path, sys.argv[1:])
rows=json.loads(native_json.read_text())["assets"]
assert len(rows)==4 and len({r["asset"] for r in rows})==4
results=[]
with zipfile.ZipFile(igz) as archive:
 for r in rows:
  encoded=archive.read(r["asset"]); image=QImage.fromData(encoded)
  assert not image.isNull() and [image.width(),image.height()]==r["dimensions"]
  rgba=image.convertToFormat(QImage.Format_RGBA8888); raw=bytes(rgba.constBits())
  native=(native_raw/r["raw_name"]).read_bytes(); assert len(raw)==len(native)
  scaled=image.scaled(4,4,Qt.IgnoreAspectRatio,Qt.SmoothTransformation)
  pixels=[list(scaled.pixelColor(x,y).getRgb()) for y in range(4) for x in range(4)]
  temporary=output.with_suffix(".qt-rgba"); assert not temporary.exists()
  try:
   temporary.write_bytes(raw)
   sampled=ast.literal_eval(subprocess.check_output([str(kernel),str(temporary),str(image.width()),str(image.height()),"false"],text=True))
  finally: temporary.unlink(missing_ok=True)
  changes=[i for i,(a,b) in enumerate(zip(raw,native)) if a!=b]
  results.append(dict(asset=r["asset"],encoded_sha256=hashlib.sha256(encoded).hexdigest(),qt_raw_sha256=hashlib.sha256(raw).hexdigest(),native_raw_sha256=hashlib.sha256(native).hexdigest(),decoded_changed_bytes=len(changes),decoded_max_diff=max(abs(a-b) for a,b in zip(raw,native)),qt_pixels=pixels,native_pixels=r["pixels"],kernel_on_qt_decoded_pixels=sampled,kernel_exact=sampled==pixels,decoded_exact=raw==native))
report=dict(qt=qVersion(),igz_sha256=hashlib.sha256(igz.read_bytes()).hexdigest(),kernel_sha256=hashlib.sha256(kernel.read_bytes()).hexdigest(),results=results,passed=all(r["kernel_exact"] and r["decoded_exact"] for r in results))
output.write_text(json.dumps(report,indent=2))
assert all(r["kernel_exact"] for r in results), "scale kernel failed on Qt inputs"
if not report["passed"]: sys.exit(1)
