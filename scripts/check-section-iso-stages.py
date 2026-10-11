#!/usr/bin/env python3
"""Strict bounded observer attribution; no release-parity promotion."""
import hashlib,json,sys,zipfile,math
from pathlib import Path
root,expected,source=map(Path,sys.argv[1:4])
candidate=len(sys.argv)>4 and sys.argv[4]=="--positive-depth-viewport"
assert hashlib.sha256(source.read_bytes()).hexdigest()=="9759c0f8a4b67a7e85268e581c3b0ffe7d477809327e46ec4b39d9790fed933f"
expected_bytes=expected.read_bytes()
assert len(expected_bytes)==1920000
assert hashlib.sha256(expected_bytes).hexdigest()=="9fabf645e1f3518902a2cfebbbae600663d48e8caf3980391d44a12313e88e55"
ordinary=(root/"closed-iso-ordinary.rgba").read_bytes()
assert len(ordinary)==1920000
assert (root/"closed-iso-observed.rgba").read_bytes()==ordinary
if not candidate: assert ordinary==expected_bytes
j=json.loads((root/"report.json").read_text());assert j["case"]=="closed-iso" and j["ordinary_and_observed_equal_authenticated_retained"] is (not candidate)
assert j["candidate_positive_depth_viewport"] is candidate
if candidate:
 d=j["positive_depth_domain"]
 assert j["warm_opt_in_refused"] is True
 assert d["enabled"] and d["all_original_and_cap_and_active_contour_clip_z_nonnegative"] and d["all_w_exactly1"] and d["x_y_w_unchanged"]
 assert d["cap_vertices"]==6 and d["viewport_depth_range"]==[0.5,1.0]
 assert j["resource"]["positive_depth_viewport_probe"] is True
assert j["resource"]["completed"] is True and j["resource"]["readback"]["shared"] is True
assert len(j["stage_color_bits"])==5 and all(len(s)==12 and all(len(v)==4 for v in s)for s in j["stage_color_bits"])
assert [p["stage"]for p in j["prefixes"]]==list(range(4))
assert [p["point_top_origin"]for p in j["pixels"]]==[[297,329],[297,328],[298,303]]
rows=[]
with zipfile.ZipFile(source)as z:
 source_bytes=z.read("closed-iso.rgba")
 assert len(source_bytes)==1920000
 full=json.loads(z.read("report.json"))
 assert full["unmodified_full_frame_exact"] is True and full["fresh_resolved_exact"] is True
 assert full["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
 assert full["actual_camera_uniform_column_major_bits"]==j["resource"]["camera_column_major_f32_bits"][0]+j["resource"]["camera_column_major_f32_bits"][1]+j["resource"]["camera_column_major_f32_bits"][2]+j["resource"]["camera_column_major_f32_bits"][3]
 for stage,prefix in enumerate(j["prefixes"]):
  assert prefix["color_matches_in_pass"] is True and prefix["resource"]["completed"] is True and prefix["resource"]["readback"]["shared"] is True
  assert prefix["color_bits"]==j["stage_color_bits"][stage]
  assert len(prefix["depth_bits"])==12 and all(math.isfinite(v)for v in prefix["depth"])
  assert len((root/f"prefix-{stage}.rgba").read_bytes())==1920000
  src=json.loads(z.read(f"{stage}/report.json"));hook=json.loads(z.read(f"{stage}/hook.json"))
  assert src["unmodified_full_frame_exact"] is True and src["terminal_prefix_stage"]==stage
  assert hook["stage"]==stage and hook["whole_paint_aborted"] is True
  assert src["actual_camera_uniform_column_major_bits"]==full["actual_camera_uniform_column_major_bits"]
  assert [p["point_top_origin"]for p in src["pixels"]]==[[297,329],[297,328],[298,303]]
  for point in range(3):
   a=src["pixels"][point]
   rows.append({"stage":stage,"point":a["point_top_origin"],"source_rgba8":a["sample_rgba8"],"native_rgba_float":prefix["colors"][point*4:point*4+4],"source_depth_bits":a["sample_depth_bits"],"native_depth_bits":prefix["depth_bits"][point*4:point*4+4]})
# A mask-only stage may change stencil but cannot alter these color/depth samples.
assert j["prefixes"][1]["color_bits"]==j["prefixes"][2]["color_bits"]
assert j["prefixes"][1]["depth_bits"]==j["prefixes"][2]["depth_bits"]
print(json.dumps({"observer_attribution_pass":True,"release_parity_claim":False,"candidate_positive_depth_viewport":candidate,"changed_pixels_vs_authenticated_native":sum(ordinary[i:i+4]!=expected_bytes[i:i+4] for i in range(0,len(ordinary),4)),"strict_source_changed_pixels":sum(ordinary[i:i+4]!=source_bytes[i:i+4] for i in range(0,len(ordinary),4)),"comparisons":rows},indent=2))
