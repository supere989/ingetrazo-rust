#!/usr/bin/env python3
"""Report actual native/Qt font identity and metric differences, no pixel waiver."""
import json,sys
from pathlib import Path
qt=json.loads((Path(sys.argv[1])/"qt-glyphs.json").read_text())
native=json.loads((Path(sys.argv[2])/"native-font.json").read_text())
assert qt["qt"]=="6.12.0" and len(qt["cases"])==4
assert [r["pixels"]for r in native]==[13,17]
rows=[]
for n in native:
 identity=n["resolved_font"]
 assert identity["physical_path"] and identity["family"] and identity["postscript"]
 assert identity["pixels"]==n["pixels"] and identity["encoded_bytes"]>0
 assert n["gray_bytes"]>0 and n["rgb_bytes"]==n["gray_bytes"]*3 and n["warm_owner"]
 for q in [q for q in qt["cases"]if q["pixels"]==n["pixels"]]:
  rows.append({"pixels":n["pixels"],"qt_requested":q["requested"],"native_identity":identity,"qt_family":q["actual_family"],"qt_style":q["actual_style"],"family_equal":identity["family"]==q["actual_family"],"advance_native_26_6":n["advance"],"advance_qt":q["advance"],"ascent_qt":q["ascent"],"descent_qt":q["descent"]})
print(json.dumps({"scope":"Platform identity/metric attribution only; Linux NVIDIA full-frame strict comparison unchanged","rows":rows},indent=2))
