#!/usr/bin/env python3
"""Strict independent DAE XML, payload, App-state and original-reader comparison."""
import argparse,copy,datetime,hashlib,json,math,xml.etree.ElementTree as ET
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
CASES=['colors-default-hole','textures-affine-planar','nested-shared-reflected','datum-names-collisions','chooser-cancel-suffix','missing-resource-write-error','basename-collision']
NS='{http://www.collada.org/2005/11/COLLADASchema}'
PHASES={name:['before','export']for name in CASES[:4]}
PHASES['basename-collision']=['before','export']
PHASES.update({'chooser-cancel-suffix':['before','cancel','suffix'],'missing-resource-write-error':['before','missing','write-error']})
CONTROLS=['actual_chooser','export_scene_readonly','document_identity_preserved','history_preserved','cancel_noop','atomic_xml_failure','resource_sidecars_owned','opaque_face_preserved']
def digest(data):return hashlib.sha256(data).hexdigest()
def safe(folder,name):
 p=Path(name)
 assert not p.is_absolute() and '..' not in p.parts,'unsafe relative output'
 result=(folder/p).resolve();assert result.is_relative_to(folder.resolve())
 return result

def exact(a,b,path='$'):
 assert type(a) is type(b),f'{path}: type'
 if isinstance(a,dict):
  assert a.keys()==b.keys(),f'{path}: keys'
  for k in a:exact(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),f'{path}: length'
  for i,(x,y) in enumerate(zip(a,b)):exact(x,y,f'{path}[{i}]')
 elif isinstance(a,float):
  assert math.isfinite(a) and math.isfinite(b) and a==b and (a!=0 or math.copysign(1,a)==math.copysign(1,b)),f'{path}: float'
 else:assert a==b,f'{path}: value'

def decode_xml(file,root=None):
 root=ET.parse(file).getroot()if root is None else root;assert root.tag==NS+'COLLADA'
 ids={}
 for e in root.iter():
  if 'id' in e.attrib:assert e.attrib['id'] not in ids,'duplicate XML ID';ids[e.attrib['id']]=e
 timestamps=[]
 def node(e):
  tag=e.tag.removeprefix(NS);text=(e.text or '').strip()
  if tag in ('created','modified'):
   stamp=datetime.datetime.fromisoformat(text.replace('Z','+00:00'));assert stamp.tzinfo is not None and stamp.utcoffset()==datetime.timedelta(0),'UTC timestamp required'
   timestamps.append({'field':tag,'raw':text});text={'dynamic_utc':tag}
  elif tag=='float_array':
   text=[float(x) for x in text.split()];assert all(math.isfinite(x) for x in text);assert len(text)==int(e.attrib['count'])
  elif tag in ('color','matrix','translate','rotate','scale'):
   text=[float(x)for x in text.split()];assert all(math.isfinite(x)for x in text)
  elif tag=='float':text=float(text);assert math.isfinite(text)
  elif tag in ('p','vcount'):text=[int(x) for x in text.split()]
  elif tag in ('longitude','latitude','altitude'):text=float(text);assert math.isfinite(text)
  for key,value in e.attrib.items():
   if key in ('source','url') and value.startswith('#'):assert value[1:] in ids,'unresolved XML reference'
  return {'tag':e.tag,'attributes':dict(e.attrib),'text':text,'children':[node(c) for c in e]}
 canonical=node(root)
 assert len(timestamps)==2 and {r['field']for r in timestamps}=={'created','modified'}
 # Validate all triangle/accessor inventories independently of paired equality.
 triangles=0
 for mesh in root.iter(NS+'mesh'):
  sources={}
  for src in mesh.findall(NS+'source'):
   array=src.find(NS+'float_array');assert array is not None
   values=[float(x)for x in (array.text or '').split()];accessor=src.find(NS+'technique_common/'+NS+'accessor');assert accessor is not None
   stride=int(accessor.get('stride','1'));count=int(accessor.get('count'));assert stride>0 and count*stride==len(values)
   sources[src.get('id')]=count
  vertices={v.get('id'):v for v in mesh.findall(NS+'vertices')}
  for tri in mesh.findall(NS+'triangles'):
   inputs=tri.findall(NS+'input');assert inputs
   offsets=[int(i.get('offset','0'))for i in inputs];assert min(offsets)>=0
   width=max(offsets)+1;tokens=[int(x)for x in (tri.findtext(NS+'p')or '').split()];count=int(tri.get('count'));assert count>=0 and len(tokens)==count*3*width
   for i in inputs:
    name=i.get('source')[1:]
    if i.get('semantic')=='VERTEX':
     vertex=vertices[name];assert len(vertex)==1 and vertex[0].get('semantic')=='POSITION';name=vertex[0].get('source')[1:]
    assert name in sources
    assert all(0<=tokens[j]<sources[name]for j in range(int(i.get('offset','0')),len(tokens),width))
   triangles+=count
 sidecars=[]
 for image in root.iter(NS+'image'):
  locator=image.findtext(NS+'init_from');assert locator
  path=safe(file.parent,locator);sidecars.append({'file':locator,'encoded_sha256':digest(path.read_bytes())if path.is_file()else None,'missing':not path.is_file()})
 return {'xml':canonical,'sidecars':sidecars,'triangles':triangles},timestamps

def scene_schema(scene):
 assert type(scene)is dict and set(scene)=={'active','loose','groups','layers','units','datum'},'reader scene inventory'
 def mesh(m):
  assert type(m)is dict and set(m)=={'points','edges','faces'},'reader mesh inventory'
  assert all(type(m[k])is list for k in ['points','edges','faces'])
  def point(p):assert type(p)is list and len(p)==3 and all(type(v)is float and math.isfinite(v)for v in p),'reader point type'
  for p in m['points']:point(p)
  for e in m['edges']:
   assert type(e)is dict and set(e)=={'points','soft','hidden'}and type(e['points'])is list and len(e['points'])==2
   for p in e['points']:point(p)
   assert type(e['soft'])is bool and type(e['hidden'])is bool
  for f in m['faces']:
   assert type(f)is dict and set(f)=={'outer','holes','attrs'}and type(f['outer'])is list and len(f['outer'])>=3 and type(f['holes'])is list and type(f['attrs'])is dict
   for p in f['outer']:point(p)
   for ring in f['holes']:
    assert type(ring)is list and len(ring)>=3
    for p in ring:point(p)
  return len(m['faces'])
 faces=mesh(scene['active'])+mesh(scene['loose'])
 def group(g):
  assert type(g)is dict and set(g)=={'name','layer','local_matrix','billboard','mesh','children'}
  assert type(g['name'])is str and (g['layer']is None or type(g['layer'])is str)
  assert type(g['local_matrix'])is list and len(g['local_matrix'])==16 and all(type(v)is float and math.isfinite(v)for v in g['local_matrix'])
  assert type(g['children'])is list
  return mesh(g['mesh'])+sum(group(c)for c in g['children'])
 assert type(scene['groups'])is list and type(scene['layers'])is list and all(type(v)is str for v in scene['layers'])and type(scene['units'])is str
 assert scene['datum']is None or type(scene['datum'])is dict
 faces+=sum(group(g)for g in scene['groups']);assert faces>0,'joint empty reader geometry'
def reader_schema(value):
 assert type(value)is dict and set(value)=={'scene','sidecars'},'reader inventory'
 scene_schema(value['scene']);assert type(value['sidecars'])is list
 for item in value['sidecars']:
  assert type(item)is dict and set(item)=={'file','resource'}and type(item['file'])is str
  r=item['resource'];assert type(r)is dict and type(r.get('missing'))is bool
  if r['missing']:assert r=={'encoded_sha256':None,'missing':True}
  else:
   assert set(r)=={'encoded_sha256','missing','pixels','rgba_sha256'}
   assert all(type(r[k])is str and len(r[k])==64 and all(c in '0123456789abcdef'for c in r[k])for k in ['encoded_sha256','rgba_sha256'])
   assert type(r['pixels'])is list and len(r['pixels'])==2 and all(type(v)is int and v>0 for v in r['pixels'])

def compare(source,native,source_dir,native_dir,reopens):
 assert source['source_pin']==native['source_pin']==PIN
 assert [c['name']for c in source['cases']]==[c['name']for c in native['cases']]==CASES
 exact(source['cases'],native['cases'],'cases')
 for case in source['cases']:
  assert case['phases']==PHASES[case['name']]
  phase='export'if PHASES[case['name']]==['before','export']else ('suffix'if case['name']=='chooser-cancel-suffix'else'missing')
  file='exports/'+case['name']+('-coerced.dae'if phase=='suffix'else'-export.dae')
  assert case['exports']==[{'phase':phase,'file':file}]
  assert case['saved']==case['name']+'-saved.igz'and case['input']==case['name']+'-input.igz'
  assert case['initial_file']=='fixture-project.igz'and case['initial_dirty']is True
 expected=[(c['name'],p)for c in source['cases']for p in c['phases']]
 assert len(expected)==16 and [(s['case'],s['phase'])for s in source['states']]==[(s['case'],s['phase'])for s in native['states']]==expected
 exact(source['states'],native['states'],'states')
 rows=[]
 for case in source['cases']:
  for export in case['exports']:
   a,ta=decode_xml(safe(source_dir,export['file']));b,tb=decode_xml(safe(native_dir,export['file']));exact(a,b,'XML '+case['name'])
   rows.append({'case':case['name'],'phase':export['phase'],'file':export['file'],'triangles':a['triangles'],'source_timestamps':ta,'native_timestamps':tb,'sidecars':a['sidecars']})
 assert len(rows)==7 and sum(r['triangles']for r in rows)>0,'empty joint output'
 assert len(reopens)==len(rows)
 assert [(r['case'],r['phase'])for r in reopens]==[(r['case'],r['phase'])for r in rows]
 for row in reopens:
  assert set(row)=={'case','phase','original','native','saved_original','saved_native'}
  reader_schema(row['original']);reader_schema(row['native']);scene_schema(row['saved_original']);scene_schema(row['saved_native'])
  exact(row['original'],row['native'],'reader '+row['case'])
  exact(row['saved_original'],row['saved_native'],'saved IGZ '+row['case'])
 for report,field in [(source,'source_timestamps'),(native,'native_timestamps')]:
  bounds=report['export_time_bounds'];expected_calls=[(c['name'],p)for c in report['cases']for p in c['phases']if p!='before']
  assert [(r['case'],r['phase'])for r in bounds]==expected_calls
  for row in rows:
   bound=next(b for b in bounds if (b['case'],b['phase'])==(row['case'],row['phase']))
   assert type(bound['started'])in(int,float)and type(bound['ended'])in(int,float)and math.isfinite(bound['started'])and math.isfinite(bound['ended'])and bound['started']<=bound['ended']
   for stamp in row[field]:
    seconds=datetime.datetime.fromisoformat(stamp['raw'].replace('Z','+00:00')).timestamp()
    assert bound['started']-1<=seconds<=bound['ended']+1,'UTC stamp outside actual export call (second granularity)'
 assert source['callback_errors']==[]
 controls=native['controls'];assert set(controls)==set(CONTROLS)and all(type(v)is bool and v for v in controls.values()),'operative controls must be literal true'
 return rows

def main():
 ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('native',type=Path);ap.add_argument('comparison',type=Path);args=ap.parse_args()
 report={'result':'fail','source_captures':0,'scope':'Strict complete App states, ordered XML semantics/indices/material/UV, encoded sidecars and actual original reader. Only raw UTC created/modified timestamps are separately validated; no geometry tolerance or directed winding exemption.'}
 try:
  source=json.loads((args.source/'original-dae-export.json').read_text());native=json.loads((args.native/'native-dae-export.json').read_text());readers=json.loads((args.native/'native-dae-reopens.json').read_text())
  report['comparisons']=compare(source,native,args.source,args.native,readers)
  probes=[]
  for name,mutate in [('empty-cases',lambda n:n.update(cases=[])),('empty-states',lambda n:n.update(states=[])),('wrong-pin',lambda n:n.update(source_pin='wrong')),('numeric-control',lambda n:n['controls'].update({next(iter(n['controls'])):1})),('false-control',lambda n:n['controls'].update({next(iter(n['controls'])):False})),('truncated-states',lambda n:n['states'].pop()),('duplicate-case',lambda n:n['cases'].append(n['cases'][0])),('changed-outcome',lambda n:n['states'][0].update(outcome='corrupt'))]:
   candidate=copy.deepcopy(native);mutate(candidate)
   try:compare(source,candidate,args.source,args.native,readers)
   except (AssertionError,KeyError,ValueError,TypeError):probes.append(name)
   else:raise AssertionError('negative escaped: '+name)
  for name,mutation in [('joint-empty-reader-observations',lambda rows:[r.update(original={},native={},saved_original={},saved_native={})for r in rows]),('missing-reader-scene-key',lambda rows:[r['original']['scene'].pop('active')for r in rows])]:
   wrong=copy.deepcopy(readers);mutation(wrong)
   try:compare(source,native,args.source,args.native,wrong)
   except (AssertionError,KeyError,ValueError,TypeError):probes.append(name)
   else:raise AssertionError('negative escaped: '+name)
  for case in source['cases']:
   file=safe(args.native,case['exports'][0]['file']);baseline,_=decode_xml(file);root=ET.parse(file).getroot()
   probes_for_case=[]
   arrays=list(root.iter(NS+'float_array'))
   color=next(root.iter(NS+'color'),None)
   if color is not None:
    wrong=copy.deepcopy(root);c=next(wrong.iter(NS+'color'));parts=c.text.split();parts[0]=str(float(parts[0])+0.1);c.text=' '.join(parts);probes_for_case.append(('changed-diffuse-color',wrong))
   for semantic,label in [('NORMAL','changed-normal-source'),('TEXCOORD','changed-UV-source')]:
    inp=next((i for i in root.iter(NS+'input')if i.get('semantic')==semantic),None)
    if inp is not None:
     wrong=copy.deepcopy(root);sid=inp.get('source')[1:];array=next(e for e in wrong.iter(NS+'source')if e.get('id')==sid).find(NS+'float_array');parts=array.text.split();parts[0]=str(float(parts[0])+0.125);array.text=' '.join(parts);probes_for_case.append((label,wrong))
   if arrays:
    wrong=copy.deepcopy(root);array=next(wrong.iter(NS+'float_array'));parts=array.text.split();parts[0]='nan';array.text=' '.join(parts);probes_for_case.append(('nonfinite-XML-source',wrong))
   tri=next(root.iter(NS+'triangles'),None)
   if tri is not None:
    wrong=copy.deepcopy(root);t=next(wrong.iter(NS+'triangles'));p=t.find(NS+'p');parts=p.text.split();parts[0]='999999999';p.text=' '.join(parts);probes_for_case.append(('out-of-range-XML-index',wrong))
   material=next(root.iter(NS+'instance_material'),None)
   if material is not None:
    wrong=copy.deepcopy(root);next(wrong.iter(NS+'instance_material')).set('target','#missing');probes_for_case.append(('unresolved-material-binding',wrong))
   image=next(root.iter(NS+'image'),None)
   if image is not None:
    wrong=copy.deepcopy(root);next(wrong.iter(NS+'image')).find(NS+'init_from').text='../escape.png';probes_for_case.append(('sidecar-path-escape',wrong))
   for name,tree in probes_for_case:
    label=case['name']+':'+name
    try:changed,_=decode_xml(file,tree);exact(baseline,changed,'mutated XML')
    except (AssertionError,KeyError,ValueError,TypeError):probes.append(label)
    else:raise AssertionError('negative escaped: '+label)
  report.update(result='pass',cases=7,states=len(source['states']),reader_pairs=len(readers),negative_controls=probes)
 except Exception as e:report['error']=repr(e)
 args.comparison.parent.mkdir(parents=True,exist_ok=True);args.comparison.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));return 0 if report['result']=='pass'else 1
if __name__=='__main__':raise SystemExit(main())
