#!/usr/bin/env python3
"""Freeze actual QColor long-hex channel decoding, without modifying source."""
from pathlib import Path
import json,hashlib
from PySide6.QtGui import QColor
out=Path("docs/audits/sheet-shape-original");out.mkdir(parents=True,exist_ok=True);data=bytearray()
for digits in [3,4]:
 for n in range(1<<(digits*4)):
  color=QColor("#"+f"{n:0{digits}x}"*3);assert color.isValid();data.extend(color.getRgb())
(out/"original-long-hex-rgba.bin").write_bytes(data)
(out/"long-hex-provenance.json").write_text(json.dumps({"source":"Actual original installed PySide6 QColor","channel_values":4096+65536,"formats":["#RRRGGGBBB","#RRRRGGGGBBBB"],"sha256":hashlib.sha256(data).hexdigest(),"boundary":"Exhaustive grayscale channel decoding, not image raster parity"},indent=2)+"\n")
print("Actual QColor69632 channel values retained")
