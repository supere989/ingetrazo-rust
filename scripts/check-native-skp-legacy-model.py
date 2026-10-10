#!/usr/bin/env python3
"""Strict imported structures and annotation inventory comparison of real pinned legacy models."""
import sys,json,hashlib,subprocess,gzip
from pathlib import Path
root=Path(__file__).resolve().parents[1];oracle=Path(sys.argv[1]);binary=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
parser_pin='291700bc213655a00461838de71e5f206311e619';app_pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()==parser_pin
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==app_pin
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from core.scene import Scene
from formats.skp import load_skp
from formats.igz import load_into
# Reuse the established strict whole-document signature, without running its corpus.
script=(root/'scripts/check-native-skp-vff.py').read_text();exec(script[script.index('def ring('):script.index('rows=[]')])
def inventory(s):
 return {'root_mesh':{'faces':sorted(face(f)for f in s.mesh.faces),'edges':sorted(edge(e)for e in s.mesh.edges),'vertices':sorted(v.position.toTuple()for v in s.mesh.vertices)},
 'dimensions':[(d.a.toTuple(),d.b.toTuple(),d.offset.toTuple(),d.axis,d.layer,d.text, d.anchor_a is not None,d.anchor_b is not None)for d in s.dimensions],
 'texts':[(t.anchor.toTuple(),t.offset.toTuple(),t.text)for t in s.text_labels],
 'saved_views':[v.to_dict()for v in s.saved_views],
 'section_planes':[v.to_dict()for v in s.section_planes],
 'images':[v.to_dict()for v in s.image_planes],
 'guides_count':len(s.guides),'back_face_color':s.back_face_color,'dimension_style':s.dimension_style}
rows=[]
for name in ['gondola_v20','capilla_quiroz_v17','legacy_v7_synthetic']:
 path=oracle/'packages/python/tests/fixtures'/(name+'.skp');output=out/(name+'.igz');output.unlink(missing_ok=True)
 command=[str(binary),str(path),str(output),str(out/(name+'-payload.json'))]
 result=subprocess.run(command,capture_output=True,text=True,timeout=30);(out/(name+'.log')).write_text(result.stdout+result.stderr);assert result.returncode==0,result.stderr
 source=Scene();load_skp(source,path);native=Scene();load_into(native,output)
 a=scene(source);b=scene(native);a['imported_field_inventory']=inventory(source);b['imported_field_inventory']=inventory(native);raw=(json.dumps({'original':a,'native':b},separators=(',',':'))+'\n').encode();(out/(name+'-comparison.json.gz')).write_bytes(gzip.compress(raw,mtime=0));assert a==b,'strict whole-document mismatch retained'
 original=json.loads(json.dumps(a));mesh=source.groups[0].mesh;mesh.vertices[0].position.setX(mesh.vertices[0].position.x()+1);assert scene(source)!= {k:v for k,v in b.items()if k!='imported_field_inventory'}
 rows.append({'name':name,'passed':True,'input_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'groups':len(a['groups']),'materials':len(a['materials']),'inventory':a['imported_field_inventory'],'comparison_raw_sha256':hashlib.sha256(raw).hexdigest(),'negative':'altered geometry rejected'})
report={'passed':True,'cases':rows,'parser_pin':parser_pin,'app_pin':app_pin,'native_source':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
