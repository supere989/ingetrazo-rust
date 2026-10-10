#!/usr/bin/env python3
"""Actual pinned legacy source import comparison; authored controls are not era corpus closure."""
from pathlib import Path
import sys,subprocess,json,hashlib,copy
root=Path(__file__).resolve().parents[1];oracle,binary,inputs,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
parserpin='291700bc213655a00461838de71e5f206311e619';apppin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()==parserpin
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==apppin
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from core.scene import Scene
from formats.skp import load_skp,NeedsConverter
from formats.igz import load_into
from openskp import legacy
mandatory=['legacy-v17-painted-quad','legacy-v17-painted-hole','legacy-v17-two-faces','legacy-v17-hidden-face','legacy-v17-reversed-coedges','legacy-v17-null-list-item']
manifest=json.loads((inputs/'manifest.json').read_text());assert [x['name']for x in manifest['cases']]==mandatory
fixture=oracle/'packages/python/tests/fixtures/single_material_v17.skp';assert hashlib.sha256(fixture.read_bytes()).hexdigest()==manifest['source_sha256']
def ring(points):
 points=[tuple(v.position.toTuple())for v in points];return min(tuple(points[i:]+points[:i])for i in range(len(points)))
def signature(scene):
 return {'groups':[{'name':g.name,'layer':g.layer,'faces':sorted((ring(f.loop),tuple(sorted(ring(h)for h in(f.hole_loops or []))),json.dumps(f.attrs,sort_keys=True))for f in g.mesh.faces),'edges':sorted((tuple(sorted([tuple(e.v0.position.toTuple()),tuple(e.v1.position.toTuple())])),e.soft)for e in g.mesh.edges),'counts':[len(g.mesh.vertices),len(g.mesh.edges),len(g.mesh.faces)],'children':len(g.children)}for g in scene.groups],'materials':{n:m.to_dict()for n,m in scene.materials.items()},'layers':[(l.name,l.visible)for l in scene.layers]}
rows=[]
for row in manifest['cases']:
 name=row['name'];path=inputs/(name+'.skp');assert hashlib.sha256(path.read_bytes()).hexdigest()==row['sha256'];output=out/(name+'.igz');output.unlink(missing_ok=True)
 command=[str(binary),str(path),str(output),str(out/(name+'-payload.json'))];p=subprocess.run(command,capture_output=True,text=True,timeout=30);(out/(name+'.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stderr)
 source=Scene();load_skp(source,path);native=Scene();load_into(native,output);a=signature(source);b=signature(native);(out/(name+'-comparison.json')).write_text(json.dumps({'original':a,'native':b},indent=2)+'\n');assert a==b,(name,'retained mismatch')
 rows.append({'name':name,'passed':True,'faces':sum(len(g.mesh.faces)for g in source.groups)})
# Real fixture is empty: raw pinned OpenSKP metadata can parse, but actual application refuses it.
a,entities,layers,materials=legacy._walk(fixture.read_bytes());assert not entities
p=subprocess.run([str(binary),str(fixture),str(out/'empty.igz'),str(out/'empty-payload.json')],capture_output=True,text=True,timeout=30);(out/'empty-native.log').write_text(p.stdout+p.stderr);assert p.returncode!=0 and not(out/'empty.igz').exists()
try:load_skp(Scene(),fixture)
except NeedsConverter:pass
else:raise AssertionError('source empty fixture unexpectedly imported')
payload=json.loads((out/'empty-payload.json').read_text());assert payload['root']['faces']=={} and payload['material_names']=={str(s):v['name']for s,v in materials};assert payload['layers']=={str(s):{'name':v['name'],'visible':not bool(v['hidden'])}for s,v in layers}
assert payload['materials']['RedMat']['color']==[1.,0.,0.,1.]
# A literal changed world point must fail the strict signature.
source.groups[0].mesh.vertices[0].position.setX(source.groups[0].mesh.vertices[0].position.x()+1);assert signature(source)!=b
report={'passed':True,'source_parser':parserpin,'source_app':apppin,'native_source':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'cases':rows,'empty_original_and_native_refusal':True,'raw_empty_metadata':True,'negative':'altered world coordinate rejected','scope':'Actual legacy MFC primitive controls + real empty v17 raw metadata/refusal, not real legacy geometry era coverage.'}
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
