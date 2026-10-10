#!/usr/bin/env python3
"""Strict actual IGZ map metadata plus actual original reopen; no fetch/render claim."""
import copy,json,os,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core.scene import Scene
from formats import igz
original,native,saved=Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3])
a=json.loads((original/"original.json").read_text()); b=json.loads(native.read_text())
def equal(left,right):
 assert set(left)==set(right)=={"cases"}
 assert len(left["cases"])==len(right["cases"])==23
 assert left==right,"Actual source/native map metadata mismatch"
equal(a,b)
assert sorted(p.name for p in saved.glob("*.igz"))==sorted(f"case-{i}.igz" for i in range(23))
for i,row in enumerate(a["cases"]):
 scene=Scene(); igz.load_into(scene,saved/f"case-{i}.igz")
 actual=None if scene.tile_layer is None else scene.tile_layer.to_dict()
 assert actual==row["tile_capture"],(i,actual,row["tile_capture"])
controls=[]
for kind in ("missing-case","custom-provider-coercion","numeric-conversion","iterable-patches"):
 bad=copy.deepcopy(b)
 if kind=="missing-case":bad["cases"].pop()
 elif kind=="custom-provider-coercion":bad["cases"][14]["tile_capture"]["name"]="None"
 elif kind=="numeric-conversion":bad["cases"][15]["tile_capture"]["zoom"]=1
 else:bad["cases"][18]["tile_capture"]["patches"]=[[0,0,0,0]]
 try:equal(a,bad)
 except AssertionError:controls.append(kind)
 else:raise AssertionError("Negative accepted: "+kind)
print(json.dumps({"passed":True,"load_cases":23,"actual_original_reopens":23,"rejected_controls":controls,"scope":"source map metadata only; broad numeric/provider/fetch/cache/UI/GPU required"}))
