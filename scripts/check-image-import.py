#!/usr/bin/env python3
"""Strict frozen actual Image App/resource/history and original-reader comparison."""
import copy,hashlib,json,math,sys,subprocess,zipfile
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
NAMES=['png-drag-transparency','nested-width-cm','jpeg-exif-pair','webp-reuse-names','bmp-free-aspect','tiff16-orientation','cancel-invalid-escape','oblique-face-width']
HEADERS=['base.jpg','invalid.png','scan-exif.jpg','scan.bmp','scan.png','scan.webp','scan16.tif']
GUARDS=['actual_picker','actual_pointer','actual_vcb','modal_ownership','external_independence','resource_reuse','refusal_preserved','undo_armed_resource','idle_readonly']
def exact(a,b,path=''):
 assert type(a)is type(b),(path,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys',sorted(a),sorted(b))
  for key in a:exact(a[key],b[key],path+'/'+key)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,path+'/'+str(i))
 else:
  assert a==b,(path,a,b)
  if isinstance(a,float):
   assert math.isfinite(a)and math.isfinite(b),(path,'nonfinite')
   if a==0:assert math.copysign(1,a)==math.copysign(1,b),(path,'signedzero')
def file(root,name):
 assert isinstance(name,str)and name
 p=(root/name).resolve();assert p.is_relative_to(root),(name,'pathescape');assert p.is_file(),str(p);return p
def sha(path):
 h=hashlib.sha256()
 with path.open('rb')as stream:
  for b in iter(lambda:stream.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def canonical_source(a,source,native):
 mapping=json.loads(file(native,'source-material-observations.json').read_text())
 assert mapping['source_pin']==PIN
 materials=mapping['materials'];used={}
 result=copy.deepcopy(a)
 def walk(value,case):
  if isinstance(value,list):
   for item in value:walk(item,case)
  elif isinstance(value,dict):
   texture=value.get('texture')
   if isinstance(texture,dict)and isinstance(texture.get('path'),str):
    path=texture['path'];assert path in materials,('unboundsourcegeometryresource',path)
    used.setdefault(path,set()).add(case);texture['path']=copy.deepcopy(materials[path]['resource'])
   for item in value.values():walk(item,case)
 for state in result['states']:walk(state['scene']['geometry'],state['case'])
 assert materials.keys()==used.keys(),'materialmappinginventory'
 original=(Path(__file__).resolve().parents[1]/'ingetrazo').resolve()
 cases={c['name']:c for c in a['cases']}
 for raw,row in materials.items():
  assert row['original_commit']==PIN
  path=row['original_path'];assert isinstance(path,str)and not Path(path).is_absolute()and '..'not in Path(path).parts
  encoded=subprocess.check_output(['git','-C',str(original),'show',PIN+':'+path])
  assert hashlib.sha256(encoded).hexdigest()==row['resource']['encoded_sha256'],('originalmaterialencoded',raw)
  resource=row['resource'];assert resource.keys()=={'encoded_sha256','rgba_sha256','size'}
  assert len(resource['size'])==2 and all(type(v)is int and v>0 for v in resource['size'])
  assert len(resource['rgba_sha256'])==64
  decoded=file(native,row['decoded_rgba']);assert decoded.stat().st_size==resource['size'][0]*resource['size'][1]*4
  assert sha(decoded)==resource['rgba_sha256'],('originalmaterialQtbytes',raw)
  members=row['input_members'];assert {v['case']for v in members}==used[raw],('materialcasecoverage',raw)
  for entry in members:
   assert entry['input']==cases[entry['case']]['input']
   with zipfile.ZipFile(file(source,entry['input']))as archive:
    data=archive.read(entry['member'])
   assert hashlib.sha256(data).hexdigest()==entry['sha256']==resource['encoded_sha256'],('materialinputbytes',entry['case'])
 return result
def state_rows(rows):
 result=copy.deepcopy(rows)
 for row in result:
  value=row['camera']['distance'];assert type(value)in(int,float),'cameradistancenumbertype'
  number=float(value);assert math.isfinite(number)and number>0,'cameradistancefinitepositive'
  if type(value)is int:assert int(number)==value,'cameradistanceintegerrepresentation'
  row['camera']['distance']=number
 return result
def metadata(a,b):
 assert a['source_pin']==b['source_pin']==PIN
 assert a['callback_errors']==[]
 assert [c['name']for c in a['cases']]==NAMES
 expected=[]
 for c in a['cases']:
  phases=['before']+[v['phase']for v in c['actions']]+['external-removed']+(['finish']if c['edit_path']else[])+['save']
  exact(phases,c['phases'],c['name']+'/sourcephaseinventory')
  expected.extend((c['name'],phase)for phase in phases)
 assert len(expected)==len(a['states'])==114,'frozen114stateinventory'
 assert [(v['case'],v['phase'])for v in a['states']]==expected,'sourceinventory'
 exact([{k:c[k]for k in ['name','saved','phases','cache_reuse']}for c in a['cases']],b['cases'],'cases')
 exact(state_rows(a['states']),state_rows(b['states']),'states')
 assert [v['name']for v in a['headers']]==HEADERS
 exact(a['headers'],b['headers'],'headers')
 assert [(v['case'],v['phase'])for v in a['dialogs']]==[('cancel-invalid-escape','invalid-header')],'sourceactualerrorinventory'
 for key in GUARDS:assert b['guards'].get(key)is True,('guard',key)
def readers_check(readers):
 assert [v['case']for v in readers]==NAMES,'readerinventory'
 for row in readers:exact(row['original'],row['native'],'reader/'+row['case'])
def bytes_check(source,native,a):
 for row in a['headers']:
  assert sha(file(source,row['file']))==row['encoded_sha256'],('sourceencoded',row['name'])
  decoded=row['decoded']
  if decoded is None:continue
  x=file(source,decoded['rgba']);y=file(native,decoded['rgba']);expected=decoded['size'][0]*decoded['size'][1]*4
  assert x.stat().st_size==y.stat().st_size==expected,('decodedbytesize',row['name'])
  assert sha(x)==sha(y)==decoded['sha256'],('decodedbytes',row['name'])
def rejected(call,label):
 try:call()
 except AssertionError:return
 raise AssertionError(('negativeaccepted',label))
def negatives(a,b,readers):
 probes=[]
 for key in ['undo','redo']:
  x=copy.deepcopy(b);x['states'][0][key]+=1;probes.append((key,x))
 x=copy.deepcopy(b);x['states'].pop();probes.append(('stateinventory',x))
 x=copy.deepcopy(b);x['cases'].reverse();probes.append(('caseorder',x))
 x=copy.deepcopy(b);x['states'][0]['scene']={'corrupt':True};probes.append(('geometry',x))
 x=copy.deepcopy(b);x['states'][0]['camera']['distance']+=1.;probes.append(('camera',x))
 for value in [True,float('inf'),float('nan')]:
  x=copy.deepcopy(b);x['states'][0]['camera']['distance']=value;probes.append(('distance-domain/'+repr(value),x))
 x=copy.deepcopy(b);x['states'][1]['tool']['armed']=not x['states'][1]['tool']['armed'];probes.append(('toolownership',x))
 x=copy.deepcopy(b);x['states'][1]['tool']['aspect']+=.125;probes.append(('toolaspect',x))
 placed=next(i for i,v in enumerate(b['states'])if v['scene']['images'])
 for key,value in [('origin',[123.,456.,789.]),('name','corrupt')]:
  x=copy.deepcopy(b);x['states'][placed]['scene']['images'][0][key]=value;probes.append(('image'+key,x))
 x=copy.deepcopy(b);x['states'][placed]['scene']['images'][0]['texture']['path']['encoded_sha256']='0'*64;probes.append(('encodedresource',x))
 x=copy.deepcopy(b);x['headers'][0]['aspect']+=.125;probes.append(('headeraspect',x))
 x=copy.deepcopy(b);x['headers'][0]['decoded']['sha256']='0'*64;probes.append(('headerpixels',x))
 x=copy.deepcopy(b);x['cases'][3]['cache_reuse']['same_name_equal']=False;probes.append(('cacheidentity',x))
 for key in GUARDS:
  x=copy.deepcopy(b);x['guards'][key]=False;probes.append(('guard/'+key,x))
 for label,x in probes:rejected(lambda:metadata(a,x),label)
 x=copy.deepcopy(readers);x.pop();rejected(lambda:readers_check(x),'readerinventory')
 x=copy.deepcopy(readers);x[0]['native']['image_planes'][0]['origin']=[123.,456.,789.];rejected(lambda:readers_check(x),'readergeometry')
 x=copy.deepcopy(readers);x[0]['native']['image_planes'][0]['texture']['path']['encoded_sha256']='0'*64;rejected(lambda:readers_check(x),'readerresource')
 return len(probes)+3
s=Path(sys.argv[1]).resolve();n=Path(sys.argv[2]).resolve();report=Path(sys.argv[3]);result={'ok':False}
try:
 a=json.loads(file(s,'original-image-import.json').read_text());b=json.loads(file(n,'native-image-import.json').read_text());a=canonical_source(a,s,n);metadata(a,b)
 result.update(cases=len(a['cases']),states=len(a['states']),headers=len(a['headers']),literal_states=True)
 bytes_check(s,n,a);result['decoded_bytes']=True
 readers=json.loads(file(n,'reopens-image-import.json').read_text());readers_check(readers);result['readers']=len(readers)
 for c in b['cases']:file(n,c['saved'])
 count=negatives(a,b,readers);result.update(ok=True,negative_controls=count,guards=GUARDS,scope='Literal App/model/transient tool/history/resource bytes; camera.distance accepts only exact finite positive int/f64 numeric equality with lossless integer representation; all other types and values remain strict and actual original IGZ readers. Source error-message text diagnostic only; pinned original/input-bound material locator-to-resource identity supplement preserves all geometry metadata. Required operative native modal/refusal guards. No framebuffer/GPU/general-codec claim.')
except Exception as e:result['error']=repr(e)
report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));sys.exit(0 if result['ok']else 1)
