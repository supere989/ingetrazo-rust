"""Strict finite-source fields and persistent App history; no tolerance."""
import copy,hashlib,itertools,json,sys,zipfile
from pathlib import Path
root=Path(sys.argv[1])
with zipfile.ZipFile(root/'records.zip') as z:
 data={n:json.loads(z.read(n))for n in ['source.json','native.json','app-native.json']}
 assert hashlib.sha256(z.read('source.json')).hexdigest()=='d56003f907dd01afaf2d3c4576bb505bce4f81cf71ccc77ad6ed5e2471b89f47'
source=data['source.json'];native=data['native.json'];app=json.loads(Path(sys.argv[3]).read_text()) if len(sys.argv)>3 else data['app-native.json']
workflows=[('roof-thicken-flush',2)]+[('wedge-'+''.join(map(str,p)),3)for p in itertools.permutations(range(3))]+[('prism-'+''.join(map(str,p)),len(p))for k in [1,2,3]for p in itertools.permutations(range(3),k)]+[('repeat-'+''.join(map(str,p)),3)for p in [(0,0,1),(1,1,2),(2,2,0),(0,1,0)]]
required=[name+'/'+str(i)for name,count in workflows for i in range(count)]
assert len(required)==65 and len(workflows)==26
assert [w['workflow']for w in source['workflows']]==[w for w,_ in workflows]
assert all(w['source_test_error'] is None for w in source['workflows'])
def ring(ps):return min(tuple(tuple(p)for p in ps[i:]+ps[:i])for i in range(len(ps)))
def inventory(fs):return sorted((ring(f['vertices']),tuple(sorted(ring(h)for h in f['holes'])),json.dumps(f['attrs'],sort_keys=True),f['interior'])for f in fs)
def validate(rows,history):
 assert [r['name']for r in rows]==required
 for s,n in zip(source['cases'],rows):
  assert n['error'] is None and len(n['result'])==len(s['result']),s['name']
  assert inventory(n['result'])==inventory(s['result']),s['name']
  if history:
   assert n['workflow']==s['workflow']
   assert n['commit_undo']==s['source_undo_delta']==1 and n['acquisition_undo']==0
   assert n['persistent_undo_count']==s['step']+1
   assert n['cancel_exact'] and n['undo_exact'] and n['redo_exact']
   assert inventory(n['acquired'])==inventory(n['undo'])==inventory(n['cancel'])
   assert inventory(n['result'])==inventory(n['redo'])
validate(native['cases'],False);validate(app['cases'],True)
negatives=[]
for name in ['empty','truncated','duplicate','removed_face','reversed_face','wrong_paint','wrong_interior','coordinate_2^-22','wrong_undo','lost_cancel','collapsed_history','wrong_workflow']:
 rows=copy.deepcopy(app['cases'])
 if name=='empty':rows=[]
 elif name=='truncated':rows.pop()
 elif name=='duplicate':rows[-1]=rows[0]
 elif name=='removed_face':rows[0]['result'].pop()
 elif name=='reversed_face':rows[0]['result'][0]['vertices'].reverse()
 elif name=='wrong_paint':rows[0]['result'][0]['attrs']['vendor']='lost'
 elif name=='wrong_interior':rows[0]['result'][0]['interior']=not rows[0]['result'][0]['interior']
 elif name=='coordinate_2^-22':rows[0]['result'][0]['vertices'][0][0]+=2**-22
 elif name=='wrong_undo':rows[0]['undo']=rows[0]['result']
 elif name=='lost_cancel':rows[0]['cancel_exact']=False
 elif name=='collapsed_history':rows[1]['persistent_undo_count']=1
 else:rows[0]['workflow']='other'
 try:validate(rows,True)
 except (AssertionError,KeyError,ValueError):negatives.append(name)
 else:raise AssertionError('accepted corruption '+name)
report={'passed':True,'workflows':26,'exact_primitive_operations':65,'exact_persistent_app_operations':65,'negative_controls':negatives,'scope':'Finite valid slanted roof, all wedge permutations, side-wall permutations and repeated pushes; exact source geometry/metadata and actual App VCB/Cancel/Undo/Redo. Not full P03 or renderer acceptance.'}
Path(sys.argv[2]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
