#!/usr/bin/env python3
import pathlib,json,sys
source=pathlib.Path(sys.argv[1]);native=pathlib.Path(sys.argv[2]);reference=json.loads((source/"original.json").read_text());actual=json.loads((native/"native.json").read_text());names=set(reference["cases"])
assert len(names)==6 and set(actual)==names,"Mandatory literal corpus missing"
def check(actual):
 for name,case in reference["cases"].items():
  assert all(actual[name][key]==case[key] for key in ["width","height","cutout"]),(name,case,actual[name])
  assert (native/(name+".rgba")).read_bytes()==(source/(name+".rgba")).read_bytes(),name
check(actual)
mut=json.loads(json.dumps(actual));first=sorted(names)[0];mut[first]["cutout"]=not mut[first]["cutout"]
try:check(mut)
except AssertionError:pass
else:raise AssertionError("Cutout mutation accepted")
p=native/(first+".rgba");original=p.read_bytes();p.write_bytes(bytes([original[0]^1])+original[1:])
try:
 try:check(actual)
 except AssertionError:pass
 else:raise AssertionError("Pixel mutation accepted")
finally:p.write_bytes(original)
print("PASS6 actualQt/source_get_texture decodeRGBA/cutout controls;2negative mutations rejected")
