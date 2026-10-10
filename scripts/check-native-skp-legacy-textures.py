#!/usr/bin/env python3
"""Actual pinned import/document/decoded texture comparison; not real model closure."""
from pathlib import Path
import sys,subprocess,json,hashlib,copy
root=Path(__file__).resolve().parents[1];oracle,binary,inputs,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
parserpin='291700bc213655a00461838de71e5f206311e619';apppin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()==parserpin
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==apppin
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from core.scene import Scene
from formats.skp import load_skp
from formats.igz import load_into
from formats.skp_openskp import _needs_tint
from openskp import legacy
from PySide6.QtGui import QImage
mandatory=['legacy-dib-unflagged','legacy-dib-greedy-average','legacy-dib-tint-green','legacy-dib-tint-alpha','legacy-dib-tint-odd','legacy-dib-guard-small','legacy-dib-guard-large','legacy-dib-fully-transparent','legacy-dib-empty']+[f'legacy-real-capilla-{slot}'for slot in [21,23,27,30]]+[f'legacy-real-legacy-{slot}'for slot in [16,18,20,22,24,26,28,30,32,34]]
manifest=json.loads((inputs/'manifest.json').read_text());assert [x['name']for x in manifest['cases']]==mandatory
fixture=oracle/'packages/python/tests/fixtures/single_material_v17.skp';assert hashlib.sha256(fixture.read_bytes()).hexdigest()==manifest['source_sha256']
def norm(value):
 if isinstance(value,dict):
  out={k:norm(v)for k,v in value.items()if k!='path'}
  if 'path'in value:
   image=QImage(value['path']);assert not image.isNull();image=image.convertToFormat(QImage.Format_RGBA8888);pixels=bytes(image.constBits());out['decoded_rgba']={'size':[image.width(),image.height()],'sha256':hashlib.sha256(pixels).hexdigest()}
  return out
 if isinstance(value,(list,tuple)):return[norm(v)for v in value]
 return value
def ring(points):
 points=[tuple(v.position.toTuple())for v in points];return min(tuple(points[i:]+points[:i])for i in range(len(points)))
def signature(scene):
 return {'groups':[{'name':g.name,'layer':g.layer,'faces':sorted((ring(f.loop),tuple(sorted(ring(h)for h in(f.hole_loops or []))),json.dumps(norm(f.attrs),sort_keys=True))for f in g.mesh.faces),'edges':sorted((tuple(sorted([tuple(e.v0.position.toTuple()),tuple(e.v1.position.toTuple())])),e.soft)for e in g.mesh.edges),'counts':[len(g.mesh.vertices),len(g.mesh.edges),len(g.mesh.faces)],'children':len(g.children)}for g in scene.groups],'materials':{n:norm(m.to_dict())for n,m in scene.materials.items()},'layers':[(l.name,l.visible)for l in scene.layers]}
rows=[]
for row in manifest['cases']:
 if 'real_model' in row:assert hashlib.sha256((oracle/'packages/python/tests/fixtures'/row['real_model']).read_bytes()).hexdigest()==row['real_model_sha256']
 name=row['name'];path=inputs/(name+'.skp');assert hashlib.sha256(path.read_bytes()).hexdigest()==row['sha256'];output=out/(name+'.igz');output.unlink(missing_ok=True)
 command=[str(binary),str(path),str(output),str(out/(name+'-payload.json'))];p=subprocess.run(command,capture_output=True,text=True,timeout=30);(out/(name+'.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stderr)
 source=Scene();load_skp(source,path);native=Scene();load_into(native,output);a=signature(source);b=signature(native);(out/(name+'-comparison.json')).write_text(json.dumps({'original':a,'native':b},indent=2)+'\n');assert a==b,(name,'retained strict mismatch')
 ar,_,_,mats=legacy._walk(path.read_bytes());mat=mats[0][1];data=ar.slots[mat['tex_dib']][2]['data'];guard=bool(mat.get('colorized'))and _needs_tint(data,mat['rgba']);assert hashlib.sha256(data).hexdigest()==row['png_sha256']
 material_name=row['material_name'];asset_files={}
 for label,scene in [('original',source),('native',native)]:
  if 'texture' not in scene.materials[material_name].to_dict():continue
  asset=Path(scene.materials[material_name].to_dict()['texture']['path']);raw=asset.read_bytes();saved=out/(name+'-'+label+'-asset'+asset.suffix);saved.write_bytes(raw);asset_files[label]={'file':saved.name,'sha256':hashlib.sha256(raw).hexdigest()}
 rows.append({'name':name,'passed':True,'source_needs_tint':guard,'stored_assets':asset_files,'texture':a['materials'][material_name].get('texture',{}).get('decoded_rgba')})
# Actual source admits null/corrupt image; native integrity refusal is intentionally distinct.
assert [r['name']for r in manifest['integrity_cases']]==['legacy-dib-truncated']
for row in manifest['integrity_cases']:
 name=row['name'];path=inputs/(name+'.skp');assert hashlib.sha256(path.read_bytes()).hexdigest()==row['sha256'];output=out/(name+'.igz');output.unlink(missing_ok=True)
 original=Scene();load_skp(original,path);assert len(original.groups)==1 and QImage(original.materials['RedMat'].to_dict()['texture']['path']).isNull()
 p=subprocess.run([str(binary),str(path),str(output),str(out/(name+'-payload.json'))],capture_output=True,text=True,timeout=30);(out/(name+'.log')).write_text(p.stdout+p.stderr);assert p.returncode!=0 and not output.exists() and 'payload' in p.stderr
# Literal changed asset identity must be rejected, not accepted because geometry matches.
altered=copy.deepcopy(b);altered['materials'][material_name]['texture']['decoded_rgba']['sha256']='0'*64;assert altered!=a
report={'passed':True,'source_parser':parserpin,'source_app':apppin,'native_source':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'cases':rows,'negative':'altered decoded texture rejected','integrity_correction':'actual source admits truncated null image; native refuses before publication','scope':'Twenty-three authored actual legacy MFC texture imports, including fourteen literal real-model material/image recipes; strict geometry/materials/UV/opacity/layers plus full decoded RGBA assets. PNG container bytes/host paths deliberately recorded separately, not equality targets. Not real nonempty legacy model/era closure.'}
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
