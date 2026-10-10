#!/usr/bin/env python3
"""Strict fixed opaque texture/font/scissor GPU diagnostic output acceptance."""
import argparse,json
from pathlib import Path

def expected(partial):
    out=bytearray()
    for y in range(64):
        for x in range(64):
            if 8<=x<16 and 8<=y<16: c=(0,0,0,255)
            elif x>=48 or y>=48: c=(255,255,255,255)
            elif x<32 and y<32: c=(255,0,0,255)
            elif x>=32 and y<32: c=(255,255,0,255) if partial else (0,255,0,255)
            elif x<32: c=(0,0,255,255)
            else: c=(255,255,255,255)
            out.extend(c)
    return bytes(out)

def exact(blob,want):
    assert len(blob)==64*64*4,"Missing/truncated GPU RGBA image"
    assert blob==want,"GPU texture/font/scissor/partial image differs"

def negative_controls():
    image=expected(False)
    for name,broken in [("empty",b""),("missing draw",bytes([255])*len(image)),("black startup",bytes([0])*len(image)),("wrong vertical origin",b"".join(image[y*256:(y+1)*256] for y in reversed(range(64)))),("stale partial",image)]:
        try: exact(broken,expected(True) if name=="stale partial" else image)
        except AssertionError: pass
        else: raise AssertionError(f"Negative accepted: {name}")
    return 5

def validate(root):
    data=json.loads((root/"result.json").read_text())
    assert data=={"passed":True,"whole_images":2,"warm_identical_gpu_images":10,"completed_drop":True,"actual_shared_event_timeout":True,"pending_drop_retained":True,"physical_mac_claim":False},"Missing/mismatched native control protocol"
    for name,partial in [("initial",False),("partial",True)]:
        want=expected(partial)
        exact((root/f"expected-{name}.rgba").read_bytes(),want)
        exact((root/f"{name}.rgba").read_bytes(),want)
    return {"whole_images_exact":2,"negative_controls":negative_controls(),"physical_mac_claim":False}

if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("root",type=Path,nargs="?");p.add_argument("--self-test",action="store_true");args=p.parse_args()
    if args.self_test: print(json.dumps({"synthetic_negative_controls":negative_controls(),"gpu_execution":False}))
    else:
        assert args.root is not None,"Native producer output directory mandatory"
        print(json.dumps(validate(args.root),sort_keys=True))
