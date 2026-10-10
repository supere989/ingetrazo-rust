#!/usr/bin/env python3
"""Authored real VFF corner-order control: exact pinned source and compiled-old rejection."""
import sys,json,hashlib,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1];oracle=Path(sys.argv[1]);binary=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()=='291700bc213655a00461838de71e5f206311e619'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path[:0]=[str(root/'ingetrazo'),str(oracle/'packages/python/src')]
from core.scene import Scene
from formats.skp import load_skp
from formats.igz import load_into
p=(root/'scripts/check-native-skp-vff.py').read_text();exec(p[p.index('def ring('):p.index('rows=[]')])
fixture=root/'docs/audits/skp-native-legacy/corner-order-proof/back-only-neighbour-weld.skp';manifest=json.loads(fixture.with_name('input.json').read_text());assert hashlib.sha256(fixture.read_bytes()).hexdigest()==manifest['sha256']
output=out/'fixed.igz';output.unlink(missing_ok=True);r=subprocess.run([str(binary),str(fixture),str(output),str(out/'payload.json')],capture_output=True,text=True,timeout=30);(out/'native.log').write_text(r.stdout+r.stderr);assert r.returncode==0,r.stderr
s=Scene();load_skp(s,fixture);n=Scene();load_into(n,output);a,b=scene(s),scene(n);(out/'comparison.json').write_text(json.dumps({'original':a,'native':b},indent=2));assert a==b
old=json.loads(fixture.with_name('compiled-old-comparison.json').read_text());assert json.loads(json.dumps(a))==old['original'];assert old['original']!=old['native']
(out/'report.json').write_text(json.dumps({'passed':True,'input_sha256':manifest['sha256'],'native_source':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'negative':'compiled-old differing representative rejected'},indent=2)+'\n');print('back-only corner ordering exact; compiled-old negative retained')
