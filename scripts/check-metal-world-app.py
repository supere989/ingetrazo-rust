#!/usr/bin/env python3
"""Strict whole-app offscreen protocol; synthetic egui input, no OS gesture claim."""
import json, pathlib, sys
NAMES = ["initial", "warm1", "warm2", "camera", "model", "select-press", "selected", "clear-press", "cleared", "alpha", "restored"]
def validate(root):
    r=json.loads((root/"report.json").read_text())
    assert r["passed"] is True and (r["width"],r["height"])==(1280,800)
    assert (r["ledger_committed"],r["ledger_reserved"])==(0,0)
    frames=r["frames"]; assert [f["name"] for f in frames]==NAMES
    by={f["name"]:f for f in frames}
    expected_routes=[(True,0),(True,0),(True,0),(True,0),(True,0),(True,0),(False,1),(False,1),(True,0),(False,0),(True,0)]
    assert [(f["native"],f["selection_count"])for f in frames]==expected_routes
    for key in ["catalog_builds","model_refreshes","matrix_bytes","geometry_uploads"]: assert by["warm1"][key]==by["warm2"][key],key
    for name in ["initial","warm1","warm2","camera","model","cleared","restored"]:
        f=by[name]; assert f["native"] is True
        image=(root/(name+".rgba")).read_bytes(); assert len(image)==1280*800*4
        x,y=map(round,f["red_pixel"]); left,top,w,h=f["rect"]
        assert left+2<x<left+w-2 and top+2<y<top+h-2
        patch=[image[4*(yy*1280+xx):4*(yy*1280+xx)+4] for yy in range(y-2,y+3) for xx in range(x-2,x+3)]
        assert all(p[0]>240 and p[1]<15 and p[2]<15 and p[3]==255 for p in patch),name
    for f in frames: assert len((root/(f["name"]+".rgba")).read_bytes())==1280*800*4
    assert by["selected"]["selection_count"]==1 and by["selected"]["native"] is False
    assert by["cleared"]["selection_count"]==0 and by["alpha"]["native"] is False
    # The moved-frame write is64 bytes; another idle target may catch up later.
    # This is not a global-total upload claim across the target ring.
    image=(root/"model.rgba").read_bytes()
    x,y=map(round,by["camera"]["red_pixel"])
    old_patch=[image[4*(yy*1280+xx):4*(yy*1280+xx)+4] for yy in range(y-2,y+3) for xx in range(x-2,x+3)]
    assert all(p==bytes([0,0,255,255])for p in old_patch),"stale old model location"
    warm,camera,model=(by[n] for n in ["warm2","camera","model"])
    for key in ["catalog_builds","model_refreshes","matrix_bytes","geometry_uploads"]: assert camera[key]==warm[key],key
    assert model["matrix_bytes"]-camera["matrix_bytes"]==64
    for key in ["catalog_builds","geometry_uploads"]: assert model[key]==camera[key],key
    assert model["model_refreshes"]==camera["model_refreshes"]+1
    return r
if __name__=="__main__":
    validate(pathlib.Path(sys.argv[1])); print("PASS actual app offscreen protocol; native SDK/runtime only, no OS pointer/presentation claim")
