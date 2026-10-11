#!/usr/bin/env python3
"""Typed source ownership/history comparison; independent semantic corruptions."""
import copy,json,sys
from pathlib import Path
root=Path(sys.argv[1])
def strict(a,b,path='root'):
 if type(a)!=type(b):
  if isinstance(a,(int,float)) and not isinstance(a,bool) and isinstance(b,(int,float)) and not isinstance(b,bool): pass
  else:raise AssertionError(f'{path}: type mismatch')
 if isinstance(a,dict):
  assert set(a)==set(b),f'{path}: fields';[strict(a[k],b[k],path+'.'+k)for k in a]
 elif isinstance(a,list):
  assert len(a)==len(b),f'{path}: length';[strict(x,y,f'{path}[{i}]')for i,(x,y)in enumerate(zip(a,b))]
 else:assert a==b,f'{path}: {a!r} != {b!r}'
source=json.loads((root/'original-nested-import.json').read_text());native=json.loads((root/'native-nested-import.json').read_text());assert len(source['cases'])==15 and len(source['states'])==105
strict(source,native)
threshold=json.loads((root/'original-threshold.json').read_text());strict(threshold,json.loads((root/'native-threshold.json').read_text()));assert len(threshold['states'])==8
strict(json.loads((root/'original-naming.json').read_text()),json.loads((root/'native-naming.json').read_text()))
guards=json.loads((root/'guards.json').read_text());assert set(guards)=={'malformed_import_preserves_document_history_context','unit_cancel_preserves_document','root_import_survives_both_cancel_checkpoints','root_command_undo_redo_after_cancel','live_layer_preserved','automatic_name_exhaustion_preserves_document_history'} and all(v is True for v in guards.values())
mutations=[]
for index,key in [(0,'depth'),(1,'undo'),(2,'redo'),(5,'undo')]:
 x=copy.deepcopy(native);x['states'][index][key]+=1;mutations.append(x)
x=copy.deepcopy(native);x['states'][1]['scene']['active']['points'][0][0]+=1;mutations.append(x)
for case in ['leaf-parts','container-zero','root-header']:
 x=copy.deepcopy(native);r=next(r for r in x['states']if r['case']==case and r['phase']=='import');r['scene']['groups'].pop();mutations.append(x)
x=copy.deepcopy(native);r=next(r for r in x['states']if r['case']=='leaf-parts'and r['phase']=='import');r['scene']['groups'][-1]['children'].reverse();mutations.append(x)
x=copy.deepcopy(native);x['states'][0]['depth']=False;mutations.append(x)
for x in mutations:
 try:strict(source,x)
 except AssertionError:pass
 else:raise AssertionError('semantic corruption accepted')
initial=json.loads((root/'attempts/initial-f64-shareback/native-nested-import.json').read_text())
try:strict(source,initial)
except AssertionError:pass
else:raise AssertionError('retained leaf Finish precision loss accepted')
reopens_path=root/'reopens.json'
assert reopens_path.exists(),'required actual original reopen observations missing'
if reopens_path.exists():
 reopens=json.loads(reopens_path.read_text());assert [r['case']for r in reopens]==[r['name']for r in source['cases']]
 for row in reopens:strict(row['original'],row['native'])
print(json.dumps({'source_states':105,'threshold_states':8,'loaded_repeat_naming_states':5,'piece_name_controls':6,'atomic_checkpoint_guards':len(guards),'rejected_mutations':len(mutations),'retained_precision_failure_rejected':True,'actual_original_reopens':15 if reopens_path.exists()else 0,'passed':True}))
