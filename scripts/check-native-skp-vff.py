#!/usr/bin/env python3
"""Strict actual pinned OpenSKP/IngeTrazo comparison of a real SKP and record controls.
Python is frozen reference only; imported application path is native Rust.
"""
import sys,os,json,subprocess,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];oracle=Path(sys.argv[1]);binary=Path(sys.argv[2]);inputs=Path(sys.argv[3]);out=Path(sys.argv[4]);out.mkdir(parents=True,exist_ok=True)
pin='291700bc213655a00461838de71e5f206311e619'
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()==pin
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from core.scene import Scene
from formats.skp import load_skp
from formats.igz import load_into
mandatory=['hole-paint-layer','single-definition','small-two-flat','shared-eight','inherited-paint','texture-refusal']
manifest=json.loads((inputs/'manifest.json').read_text());assert [r['name']for r in manifest['cases']]==mandatory
for row in manifest['cases']:assert hashlib.sha256((inputs/(row['name']+'.skp')).read_bytes()).hexdigest()==row['sha256']
def ring(vertices):
 points=[tuple(v.position.toTuple())for v in vertices];return min(tuple(points[i:]+points[:i])for i in range(len(points)))
def face(f):return(ring(f.loop),tuple(sorted(ring(h)for h in(f.hole_loops or[]))),json.dumps(f.attrs,sort_keys=True,separators=(',',':')))
def edge(e):return(tuple(sorted((tuple(e.v0.position.toTuple()),tuple(e.v1.position.toTuple())))),e.soft)
def scene(s):
 groups=[];shares={}
 for g in s.groups:
  mesh=g.mesh;local=sorted(face(f)for f in mesh.faces);geometry=[]
  for f in mesh.faces:
   pts=[tuple((g.xform.map(v.position)if g.xform is not None else v.position).toTuple())for v in f.loop];geometry.append(min(tuple(pts[i:]+pts[:i])for i in range(len(pts))))
  groups.append({'name':g.name,'layer':g.layer,'local':local,'world':sorted(geometry),'edges':sorted(edge(e)for e in mesh.edges),'counts':[len(mesh.vertices),len(mesh.edges),len(mesh.faces)]})
  shares.setdefault(id(mesh),[]).append(g.name)
 return {'groups':sorted(groups,key=lambda g:json.dumps(g,sort_keys=True)),'shares':sorted(sorted(names)for names in shares.values()),'layers':sorted((l.name,l.visible)for l in s.layers),'materials':{k:v.to_dict()for k,v in s.materials.items()}}
rows=[]
for name,path in [('SU_File',root/'docs/audits/skp-native-vff/SU_File.skp')]+[(n,inputs/(n+'.skp'))for n in mandatory]:
 output=out/(name+'.igz');output.unlink(missing_ok=True)
 command=[str(binary),str(path),str(output),str(out/(name+'-payload.json'))];r=subprocess.run(command,capture_output=True,text=True,timeout=30)
 (out/(name+'.log')).write_text(r.stdout+r.stderr)
 if name=='texture-refusal':assert r.returncode!=0 and 'material textures' in r.stderr and not output.exists();rows.append({'name':name,'passed':True,'expected_refusal':r.stderr.strip()});continue
 assert r.returncode==0,(name,r.stderr)
 source=Scene();load_skp(source,path);native=Scene();load_into(native,output)
 a=scene(source);b=scene(native);(out/(name+'-comparison.json')).write_text(json.dumps({'original':a,'native':b},indent=2)+'\n');assert a==b,(name,'source/native document mismatch; retained')
 rows.append({'name':name,'passed':True,'groups':len(source.groups),'sharing_sizes':sorted(len(x)for x in a['shares']),'faces':sum(len(g.mesh.faces)for g in source.groups)})
# Literal altered-world geometry is not accepted by the equality law.
source.groups[0].mesh.vertices[0].position.setX(source.groups[0].mesh.vertices[0].position.x()+1)
assert scene(source)!=b
report={'passed':True,'cases':rows,'source_parser':pin,'source_app':subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip(),'native_source':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'negative':'altered source world point rejected','scope':__doc__}
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
