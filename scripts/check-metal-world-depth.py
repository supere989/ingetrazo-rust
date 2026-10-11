#!/usr/bin/env python3
"""Strict sampled interior/background GPU oracle; not a whole-image edge oracle."""
import json,math,pathlib,sys
NAMES=("far-then-near","near-then-far","depth-disabled-negative","warm-0","warm-1","warm-2","owner-refusal-preserved","map-free-old-green","map-free-new-red","material-front","material-default-back","material-override-back","hidden-line-front","hidden-line-back","monochrome-front","monochrome-back","reflected-authored-front","policy-refusal-preserved")
def validate_pixels(pixels,name,normal_override=False):
    assert len(pixels)==64*64*4,(name,"raw extent")
    material={"material-front":(255,0,0,255),"material-default-back":(255,255,0,255),"material-override-back":(0,255,0,255),"hidden-line-front":(255,0,255,255),"hidden-line-back":(255,0,255,255),"monochrome-front":(255,0,255,255),"monochrome-back":(255,255,0,255),"reflected-authored-front":(255,0,0,255),"policy-refusal-preserved":(255,0,0,255)}
    expected=material.get(name, (0,255,0,255) if normal_override or name not in ("depth-disabled-negative","map-free-new-red") else (255,0,0,255))
    interior=outside=0
    for y in range(64):
        for x in range(64):
            px=2*(x+.5)/64-1;py=1-2*(y+.5)/64
            half=(.8-py)/2
            actual=tuple(pixels[(y*64+x)*4:(y*64+x)*4+4])
            if -.77<py<.77 and abs(px)<half-.04:
                assert actual==expected,(name,x,y,actual,expected);interior+=1
            elif py<-.84 or py>.84 or abs(px)>half+.04:
                assert actual==(0,0,255,255),(name,x,y,actual,"background");outside+=1
    assert interior>1000 and outside>2000,(interior,outside)
    return {"interior_pixels":interior,"background_pixels":outside}
def main(root):
    root=pathlib.Path(root);report=json.loads((root/"report.json").read_text())
    assert report["passed"] is True and report["wrong_owner_refused"] is True and report["invalid_pose_refused"] is True
    assert report["policy_metadata_refused"] is True
    assert report["frames"]==list(NAMES),report["frames"]
    assert report["width"]==report["height"]==64 and report["pixel_encoding"]=="RGBA8Unorm opaque"
    assert report["geometry_uploads_before_warm"]==report["geometry_uploads_after_warm"]==2
    assert report["targets_before_warm"]==report["targets_after_warm"]==1
    assert report["ledger_committed_after_drop"]==report["ledger_reserved_after_drop"]==0
    frames={name:(root/f"{name}.rgba").read_bytes() for name in NAMES}
    results={name:validate_pixels(pixels,name) for name,pixels in frames.items()}
    controls={}
    for name,pixels in (("blank",bytes([0,0,255,255])*4096),("depth-disabled-as-positive",frames["depth-disabled-negative"])):
        try:validate_pixels(pixels,"far-then-near",True)
        except AssertionError:controls[name]="rejected"
        else:raise AssertionError(f"{name} negative passed")
    for label,pixels,expected_name in (("back-as-front",frames["material-default-back"],"material-front"),("override-as-default",frames["material-override-back"],"material-default-back")):
        try:validate_pixels(pixels,expected_name)
        except AssertionError:controls[label]="rejected"
        else:raise AssertionError(label)
    print(json.dumps({"scope":"actual opaque/depth, source face paint-policy winding and retained UI sampling; strict interior/background samples; edges/MSAA/textures/alpha/presentation/fullviewport not claimed","passed":True,"frames":results,"negative_controls":controls},indent=2))
if __name__=="__main__":main(sys.argv[1])
