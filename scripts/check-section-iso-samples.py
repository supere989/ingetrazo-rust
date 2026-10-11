#!/usr/bin/env python3
"""Finite observation inventory and immutable full-frame guards, not parity promotion."""
import hashlib,json,math,sys,zipfile
from pathlib import Path
native=Path(sys.argv[1]);expected=Path(sys.argv[2]).read_bytes();source_archive=Path(sys.argv[3])
assert hashlib.sha256(expected).hexdigest()=="9fabf645e1f3518902a2cfebbbae600663d48e8caf3980391d44a12313e88e55"
assert hashlib.sha256(source_archive.read_bytes()).hexdigest()=="17547dfb9a567858230325b83795dd534f0175c463300174a053c7e687e02499"
with zipfile.ZipFile(source_archive) as z:
 source=json.loads(z.read("report.json"));source_raw=z.read("closed-iso.rgba")
assert source["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9" and source["fresh_resolved_exact"] is True
assert len(expected)==len(source_raw)==1920000
for name in ["closed-iso-ordinary.rgba","closed-iso-observed.rgba"]:assert (native/name).read_bytes()==expected,name
mismatch=sum(source_raw[i:i+4]!=expected[i:i+4] for i in range(0,len(expected),4));assert mismatch==134,mismatch
report=json.loads((native/"report.json").read_text());assert report["ordinary_and_observed_equal_authenticated_retained"] is True
assert report["resource"]["completed"] is True and report["resource"]["readback"]["shared"] is True
assert report["resource"]["resolve_route"]["path"]=="same-pass-tile"
points=[[297,329],[297,328],[298,303]];assert [p["point_top_origin"] for p in source["pixels"]]==points;assert [p["point_top_origin"] for p in report["pixels"]]==points
rows=[]
for a,b in zip(source["pixels"],report["pixels"]):
 colors=b["sample_rgba_float"];depth=b["sample_depth_float"]
 assert len(colors)==len(depth)==4 and all(len(c)==4 for c in colors)
 assert all(math.isfinite(v) and 0<=v<=1 for c in colors for v in c) and all(math.isfinite(v) and 0<=v<=1 for v in depth)
 assert len(b["sample_rgba_bits"])==len(b["sample_depth_bits"])==4
 rows.append({"point":a["point_top_origin"],"source_rgba8":a["sample_rgba8"],"native_rgba8":[[round(v*255) for v in c] for c in colors],"source_depth_bits":a["sample_depth_bits"],"native_depth_bits":b["sample_depth_bits"]})
print(json.dumps({"whole_source_native_mismatches_preserved":134,"observed_native_frame_unchanged":True,"source_depth_attachment":source["depth_attachment"],"native_depth_attachment":report["resource"]["depth"],"points":rows,"scope":"Measured final samples; no cause or release acceptance implied"},indent=2))
