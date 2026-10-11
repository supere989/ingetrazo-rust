#!/usr/bin/env python3
"""Authenticate complete original-reader observations and exact leaf inventory."""
import copy,hashlib,json,sys,zipfile
from pathlib import Path
audit=Path(__file__).resolve().parents[1]/'docs/audits/document-preservation-workflow';m=json.loads((audit/'manifest.json').read_text());raw=(audit/'records.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(audit/'records.zip') as z:
 assert set(z.namelist())==set(m['records'])
 for n,v in m['records'].items():
  b=z.read(n);assert len(b)==v['bytes'] and hashlib.sha256(b).hexdigest()==v['sha256'],n
 e=json.loads(z.read('capture/expected.json'));actual=json.loads(z.read('capture/observed.json'));native=json.loads(z.read('native.json'));differences=json.loads(z.read('capture/differences.json'))
 assert not any(differences.values())
assert e['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
keys={'frames','texts','images','scalebars','nortes','leyendas','shapes','cotas','cotas_ang','cotas_rad','etiquetas','perfiles','niveles','llamadas','cajetin'};assert set(e['leaf_inventory'])==keys
if len(sys.argv)>1:actual=json.loads(Path(sys.argv[1]).read_text())
def check(a):
 assert set(a)=={f+'-'+p for f in ['retained','nested-leaves'] for p in ['edited','undo','redo']}
 assert a==e['cases']
 for name,scene in a.items():
  assert len(scene['groups'])==(2 if name.endswith('undo') else 1)
  if name.startswith('nested-leaves'):
   sheet=scene['compositions'][0]
   for key,fields in e['leaf_inventory'].items():
    item=sheet[key] if key=='cajetin' else sheet[key][0]
    assert set(fields)<=set(item),(key,set(fields)-set(item))
   assert any(f.get('holes') for f in scene['faces'])
   assert any(f.get('back') is True for f in scene['faces'])
   assert any(isinstance(f.get('back'),dict) and 'texture' in f['back'] for f in scene['faces'])
check(actual)
assert [(x['fixture'],x['phase']) for x in native['rows']]==[(f,p)for f in ['retained','nested-leaves']for p in ['edited','undo','redo']]
assert all(x['saved'] and x['clean'] for x in native['rows'])
# Exact whole-document comparison rejects changes in modeled and retained leaves.
for kind in ['group-transform','page-opaque-leaf','embedded-texture','hole','plugin']:
 changed=copy.deepcopy(actual);s=changed['nested-leaves-edited']
 if kind=='group-transform':s['groups'][0]['xform'][12]+=1
 elif kind=='page-opaque-leaf':s['compositions'][0]['texts'][0]['italic']=False
 elif kind=='embedded-texture':next(f for f in s['faces'] if isinstance(f.get('back'),dict) and 'texture' in f['back'])['back']['texture']['embed']={'sha256':'0'*64}
 elif kind=='hole':next(f for f in s['faces'] if f.get('holes'))['holes']=[]
 else:s['plugin_data']={}
 try:check(changed)
 except AssertionError:pass
 else:raise AssertionError('accepted corruption: '+kind)
print('PASS: six actual app saves,15 source page-record leaf inventories, exact original reopens and five negatives')
