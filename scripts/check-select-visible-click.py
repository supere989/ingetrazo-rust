#!/usr/bin/env python3
"""Strict read-only source/native Select workflow acceptance and compiled defect negative."""
import json,hashlib,zipfile,sys
from pathlib import Path
root=Path(sys.argv[1]);manifest=json.loads((root/"manifest.json").read_text())
archive=root/manifest["archive"];assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest["archive_sha256"]
with zipfile.ZipFile(archive) as z:
 assert set(z.namelist())==set(manifest["files"])
 for name,sha in manifest["files"].items():assert hashlib.sha256(z.read(name)).hexdigest()==sha,name
 source=json.loads(z.read("original.json"));native=json.loads(z.read("native.json"));old=json.loads(z.read("compiled-old-picker.json"))
 assert source["source_commit"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
 inventory=["soft-off","soft-on","occluded","outside","shift-remove","ctrl-add","both-remove","hidden-object-off","hidden-object-on","hidden-face-off","hidden-face-on","hidden-face-locked","hidden-face-layer-off","hidden-face-layer-restored","geometry-toggle-off","object-before-toggle","object-toggle-off","object-enter","undo-object-before","undo-object-hide","undo-object-shown","undo-object-undo","xray-below","wireframe-below","double-soft","double-visible","locked-occluder","excluded-occluder"]
 assert [r["name"] for r in source["rows"]]==inventory
 fields=["name","selection","objects","geometry","edit","group_hidden","active_vertices"]
 def compare(candidate):
  assert [r["name"] for r in candidate["rows"]]==inventory,"Missing/reordered workflow state"
  for a,b in zip(source["rows"],candidate["rows"]):
   for k in fields:assert a[k]==b[k],(a["name"],k,a[k],b[k])
  refs=["soft-off","soft-on","occluded","hidden-object-off","hidden-object-on","hidden-face-off","hidden-face-on","shift-remove","ctrl-add","both-remove"]
  assert len(candidate["ui_events"])==10
  lookup={r["name"]:r for r in source["rows"]}
  for row,name in zip(candidate["ui_events"],refs):
   assert row["selection"]==lookup[name]["selection"],("Actual egui pointer events",name)
   assert len(row["viewport"])==4 and row["viewport"][2]>row["viewport"][0] and row["viewport"][3]>row["viewport"][1]
 compare(native)
 try:compare(old)
 except AssertionError:pass
 else:raise AssertionError("Compiled literal old reference-picker accepted")
 missing={**native,"rows":native["rows"][:-1]}
 try:compare(missing)
 except AssertionError:pass
 else:raise AssertionError("Missing state accepted")
 assert b'let hit_edge = crate::base::pick_scene_edge(scene, &ctx.ray, 0.2);' in z.read("compiled-old-picker.patch")
 print("PASS 28 exact Qt/native dispatcher states, 10 actual egui pointer sequences, compiled old-picker and missing-state negatives")
