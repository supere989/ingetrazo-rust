#!/usr/bin/env python3
"""Exact stable mixed-sheet semantics; Qt unordered traversal stays diagnostic."""
import copy,json,math,sys,zipfile
from pathlib import Path
ROSTER=['all-kinds-box','mixed-ctrl-move','same-kind-properties','mixed-delete-history','group-id-and-alt-stack','locked-hidden-list','sheet-switch-history','bound-frame-followers']
FLAGS=['idle_reused','atomic_refusal','actual_pointer_ingress','foreign_ui_owned']
STATE_KEYS={'case','phase','composition','all_compositions','sheet_index','selection','panel_owner','undo','redo'}
def read_doc(path):
 data=path.read_bytes()
 if data[:2]==b'PK':
  with zipfile.ZipFile(path)as z:return json.loads(z.read('document.json'))
 return json.loads(data)
def leaf(value,path):
 for key in path.split('.'):
  value=value[int(key)]if isinstance(value,list)else value[key]
 return value
def equal(a,b,path=''):
 if isinstance(a,bool)or isinstance(b,bool):assert type(a)is bool and type(b)is bool and a==b,(path,'bool',a,b);return
 if isinstance(a,(int,float))and isinstance(b,(int,float)):assert math.isfinite(a)and math.isfinite(b)and a==b,(path,'number',a,b);return
 assert type(a)is type(b),(path,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert set(a)==set(b),(path,'keys',sorted(a),sorted(b))
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'count',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+'.'+str(i))
 else:assert a==b,(path,'value',a,b)
def typed_state(row):
 assert set(row)-{'selection_order'}==STATE_KEYS,('state keys',row.keys())
 for k in ['sheet_index','undo','redo']:assert type(row[k])is int and row[k]>=0,('integer',k)
 assert type(row['case'])is str and type(row['phase'])is str
 assert type(row['composition'])is dict and type(row['all_compositions'])is list and row['sheet_index']<len(row['all_compositions'])
 for item in row['selection']+([]if row['panel_owner']is None else[row['panel_owner']]):
  assert set(item)=={'family','index'}and type(item['family'])is str and type(item['index'])is int and item['index']>=0
 assert len({(v['family'],v['index'])for v in row['selection']})==len(row['selection'])
 assert row['selection']==sorted(row['selection'],key=lambda x:(x['family'],x['index']))
 assert row['panel_owner']is None or row['panel_owner']in row['selection']
 equal(row,row,'typed-self')
def compare(source,native,readers,docs):
 assert source['source_pin']==native['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in source['cases']]==[c['name']for c in native['cases']]==ROSTER
 assert len(source['states'])==len(native['states'])==105
 assert native['failures']==[]
 for a,b in zip(source['states'],native['states']):
  typed_state(a);typed_state(b);equal({k:a[k]for k in STATE_KEYS},{k:b[k]for k in STATE_KEYS},a['case']+'/'+a['phase'])
 assert [r['case']for r in readers]==ROSTER;equal(source['readers'],readers,'original-readers')
 assert set(native['controls'])==set(FLAGS+['cohort_cap_atomic'])
 for flag in FLAGS:assert type(native['controls'][flag])is bool and native['controls'][flag],flag
 assert type(native['controls']['cohort_cap_atomic'])is bool and not native['controls']['cohort_cap_atomic'],'unproved-cap'
 assert [r['case']for r in native['control_cases']]==ROSTER
 for row in native['control_cases']:
  for flag in FLAGS:assert type(row[flag])is bool and row[flag],('casecontrol',row['case'],flag)
  for name in['pointer_events','property_events']:assert type(row[name])is int and row[name]>=0
  assert row['pointer_events']>0
  assert type(row['cohort_cap_atomic'])is bool and not row['cohort_cap_atomic']
  assert set(row['foreground'])=={'actual_foreground_layer','pointer_down_contains','model_history_selection_unchanged'}
  for v in row['foreground'].values():assert type(v)is bool and v
 for guard in source.get('opaque_guards',[]):
  for path,expected in guard['saved_paths'].items():equal(expected,leaf(docs[guard['case']],path),'savedopaque/'+path)
 errors=source['callback_errors'];assert len(errors)==6
 assert all(e['case']=='all-kinds-box'and e['phase']=='corner-release'and e['type']=='AttributeError'and e['message']=="'ComposerWindow' object has no attribute '_single_selected'"and'in _on_cota_rad_props'in e['traceback']for e in errors)
 return {'states':105,'cases':8,'original_readers':8,'source_radial_population_faults':6}
def controls(source,native,readers,docs):
 # Each adversary starts from this same independently accepted full baseline.
 compare(source,native,readers,docs);names=[]
 def reject(name,edit):
  n=copy.deepcopy(native);r=copy.deepcopy(readers);d=copy.deepcopy(docs);edit(n,r,d)
  try:compare(source,n,r,d)
  except (AssertionError,KeyError,TypeError,ValueError):names.append(name)
  else:raise AssertionError('Inert checker mutation '+name)
 reject('truncated-roster',lambda n,r,d:n['cases'].pop())
 reject('truncated-state',lambda n,r,d:n['states'].pop())
 reject('wrong-history-type',lambda n,r,d:n['states'][0].__setitem__('undo',False))
 reject('missing-membership',lambda n,r,d:next(v for v in n['states']if len(v['selection'])>2)['selection'].pop())
 reject('lost-primary',lambda n,r,d:next(v for v in n['states']if v['panel_owner']is not None).__setitem__('panel_owner',None))
 reject('extra-known-key',lambda n,r,d:n['states'][0]['composition'].__setitem__('extra',1))
 for flag in FLAGS:
  reject('false-global-'+flag,lambda n,r,d,k=flag:n['controls'].__setitem__(k,False))
  reject('false-case-'+flag,lambda n,r,d,k=flag:n['control_cases'][0].__setitem__(k,False))
 reject('missing-control-case',lambda n,r,d:n['control_cases'].pop())
 reject('inert-pointer-count',lambda n,r,d:n['control_cases'][0].__setitem__('pointer_events',0))
 reject('bool-pointer-count',lambda n,r,d:n['control_cases'][0].__setitem__('pointer_events',True))
 reject('false-foreground',lambda n,r,d:n['control_cases'][0]['foreground'].__setitem__('actual_foreground_layer',False))
 reject('invented-cap-proof',lambda n,r,d:n['controls'].__setitem__('cohort_cap_atomic',True))
 reject('missing-reader',lambda n,r,d:r.pop())
 reject('reader-extra',lambda n,r,d:r[0]['compositions'][0].__setitem__('extra',True))
 reject('save-lost-opaque',lambda n,r,d:d['mixed-delete-history']['scene']['compositions'][0].pop('vendor_mixed'))
 reject('failed-runtime',lambda n,r,d:n['failures'].append({'error':'mutated'}))
 reject('nonfinite-coordinate',lambda n,r,d:n['states'][0]['composition']['frames'][0].__setitem__('x_mm',float('nan')))
 return names
def main():
 source_dir=Path(sys.argv[1]);native_dir=Path(sys.argv[2]);target=Path(sys.argv[3]);target.parent.mkdir(parents=True,exist_ok=True)
 source=json.loads((source_dir/'original-mixed.json').read_text());source['readers']=json.loads((source_dir/'original-reopens.json').read_text());native=json.loads((native_dir/'native-mixed.json').read_text());readers=json.loads((native_dir/'native-reopens.json').read_text());docs={}
 for case in native['cases']:
  p=Path(case['saved']);assert not p.is_absolute()and'..'not in p.parts;docs[case['name']]=read_doc(native_dir/p)
 report={'result':'FAIL','scope':'Exact sorted membership, retained panel, full known model/history/readers. Qt selectedItems order retained diagnostic only; no profile painting or cohort cap execution claim.','differences':[],'negative_controls':[]}
 # Preserve every actual phase difference even when controls/reader fail later.
 for a,b in zip(source['states'],native['states']):
  try:typed_state(a);typed_state(b);equal({k:a[k]for k in STATE_KEYS},{k:b[k]for k in STATE_KEYS},a['case']+'/'+a['phase'])
  except Exception as e:report['differences'].append({'case':a['case'],'phase':a['phase'],'error':str(e)})
 target.write_text(json.dumps(report,indent=2)+'\n')
 try:
  report.update(compare(source,native,readers,docs));report['negative_controls']=controls(source,native,readers,docs);report['result']='PASS'
 except Exception as e:report['error']=str(e)
 target.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));return0=0 if report['result']=='PASS'else 1;return return0
if __name__=='__main__':sys.exit(main())
