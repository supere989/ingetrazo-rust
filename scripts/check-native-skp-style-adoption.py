#!/usr/bin/env python3
"""Two authored style records prove first-color adoption, without a style override."""
import sys,zipfile,io,re,json,subprocess,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];oracle=Path(sys.argv[1]);binary=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()=='291700bc213655a00461838de71e5f206311e619'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from core.scene import Scene
from formats.skp import load_skp
from formats.igz import load_into
parent=oracle/'packages/python/tests/fixtures/Untitled.skp';raw=parent.read_bytes();start=raw.index(b'PK\x03\x04');src=zipfile.ZipFile(io.BytesIO(raw[start:]));target=io.BytesIO();seen=0
with zipfile.ZipFile(target,'w',zipfile.ZIP_DEFLATED)as z:
 for name in src.namelist():
  data=src.read(name)
  if name.endswith('style.xml'):
   rgb=(51,77,102)if seen==0 else(221,11,44);value=(255<<24)|(rgb[0]<<16)|(rgb[1]<<8)|rgb[2]
   text,count=re.subn(r'(<(?:\w+:)?item[^>]*id="4001"[^>]*>.*?<[^>]*variant[^>]*>)[^<]+',lambda m:m[1]+str(value-(1<<32)),data.decode(),flags=re.S);assert count==1;data=text.encode();seen+=1
  z.writestr(name,data)
assert seen==2
path=out/'two-styles.skp';path.write_bytes(raw[:start]+target.getvalue());output=out/'native.igz';output.unlink(missing_ok=True)
r=subprocess.run([str(binary),str(path),str(output),str(out/'payload.json')],text=True,capture_output=True,timeout=30);assert r.returncode==0,r.stderr
s=Scene();load_skp(s,path);n=Scene();load_into(n,output);expected=(51/255,77/255,102/255);assert s.back_face_color==n.back_face_color==expected
assert s.display_style.to_dict()==n.display_style.to_dict()
assert n.back_face_color!=(221/255,11/255,44/255)
report={'passed':True,'first_back_color':expected,'source_style_override':s.display_style.to_dict(),'native_style_override':n.display_style.to_dict(),'input_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'parent_input_sha256':hashlib.sha256(raw).hexdigest(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'negative':'second-style precedence rejected'}
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
