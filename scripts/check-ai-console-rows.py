"""Strict operative source comparison, row-copy inventory and rejecting controls."""
import copy,json,sys
from pathlib import Path
recipes=json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-rows/recipes.json').read_text());names=[r['name']for r in recipes]
def validate(source,native):
 assert [s['name']for s in source['states']]==names
 assert [s['name']for s in native['states']]==names
 assert native['states']==source['states']
 assert native['native_exports']==0 and native['row_publications']==5
 assert native['vm_history_steps']==7 and native['final_owner_charge']==0
 assert [p['name']for p in native['publications']]==['paint','pose','mixed-append','delete-old','branch']
 metrics={p['name']:p['metrics']for p in native['publications']}
 assert (metrics['paint']['vertex_rows'],metrics['paint']['edge_rows'],metrics['paint']['face_rows'],metrics['paint']['index_vertices'])==(0,0,1,0)
 assert (metrics['pose']['vertex_rows'],metrics['pose']['edge_rows'],metrics['pose']['face_rows'],metrics['pose']['index_vertices'])==(1,0,0,3)
 assert (metrics['mixed-append']['vertex_rows'],metrics['mixed-append']['edge_rows'],metrics['mixed-append']['face_rows'],metrics['mixed-append']['index_vertices'])==(3,3,2,6)
 assert metrics['delete-old']['index_vertices']==0 and metrics['delete-old']['face_rows']==2
 assert (metrics['branch']['vertex_rows'],metrics['branch']['edge_rows'],metrics['branch']['face_rows'],metrics['branch']['index_vertices'])==(1,0,1,6)
 for p in native['publications']:
  assert p['metrics']['payload_bytes']>0 and p['metrics']['compared_rows']>=7
source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());validate(source,native)
mutations=[]
for key,value in [('positions',[[99,0,0]]),('edges',[]),('faces',[]),('layers',['Wrong']),('undo',99),('redo',99),('identity_checked',False)]:
 bad=copy.deepcopy(native);bad['states'][3][key]=value;mutations.append((key,bad))
for key in ['native_exports','row_publications','vm_history_steps','final_owner_charge']:
 bad=copy.deepcopy(native);bad[key]+=1;mutations.append((key,bad))
for key in ['vertex_rows','edge_rows','face_rows','index_vertices']:
 bad=copy.deepcopy(native);bad['publications'][0]['metrics'][key]+=1;mutations.append((key,bad))
bad=copy.deepcopy(native);bad['states'].pop();mutations.append(('missing-state',bad))
bad=copy.deepcopy(native);bad['states'][1],bad['states'][2]=bad['states'][2],bad['states'][1];mutations.append(('state-order',bad))
rejected=[]
for name,bad in mutations:
 try:validate(source,bad)
 except (AssertionError,KeyError,TypeError):rejected.append(name)
 else:raise AssertionError('Accepted mutation '+name)
refusal_rejected=[]
if len(sys.argv)>3:
 def validate_refusal(value):
  assert value['real_allocator_refusals']==1
  for key in ['scene_unchanged','undo_redo_preserved','revision_unchanged','retired_scope_document_history','retry_one_command','retry_alias_undo_redo']:assert value[key] is True
  assert value['refused_scope_charge']==0 and value['final_owner_charge']==0
 refusal=json.loads(Path(sys.argv[3]).read_text());validate_refusal(refusal)
 for key in refusal:
  bad=copy.deepcopy(refusal);bad[key]=False if isinstance(bad[key],bool) else bad[key]+1
  try:validate_refusal(bad)
  except (AssertionError,KeyError,TypeError):refusal_rejected.append(key)
  else:raise AssertionError('Accepted refusal mutation '+key)
print(json.dumps(dict(states=len(native['states']),rejected=rejected,refusal_rejected=refusal_rejected),indent=2))
