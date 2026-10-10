import json,sys,copy
NAMES=['initial','shared','abort','field-abort']
def equal(a,b,path='root'):
 assert type(a)is type(b),(path,type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'inventory')
  for key in a:equal(a[key],b[key],path+'.'+key)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length')
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+'.'+str(i))
 else:assert a==b,(path,a,b)
def states(source,native):
 assert len(source)==len(native)
 for a,b in zip(source,native):equal(a,b)
def validate(source,native):
 assert [s['name']for s in source['states']]==NAMES
 assert [s['name']for s in native['states']]==NAMES
 states(source['states'],native['states'])
 for key in ('persistent_mesh_lists','real_face_vertex_identity','source_welded_vertex_reuse','detached_face_valid_after_abort','native_projection','face_live_geometry_reads','two_mesh_publication','second_owner_pressure_retains_both','private_scope_edit_survives_error','final_zero_with_cycle','shallow_existing_field_rollback','preparation_refusal_session_recovers','unwind_restores_fields_and_retires_session'):
  assert type(native[key])is bool and native[key]is True,key
 for key in ('general_existing_object_rollback','undo_redo_controller','production_enabled'):
  assert type(native[key])is bool and native[key]is False,key
 assert source['face_live_geometry_reads']is True and source['private_scope_edit_survives_error']is True and source['shallow_existing_field_rollback']is True
source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native)
rejected=[]
for name,mutation in [('position',lambda n:n['states'][1]['mesh']['vertices'][0].__setitem__(0,9.)),('loop_wrapper',lambda n:n['states'][2].__setitem__('face_loop_preserved',True)),('attrs_wrapper',lambda n:n['states'][2].__setitem__('attrs_dict_preserved',True)),('position_wrapper',lambda n:n['states'][2].__setitem__('vertex_position_preserved',True)),('missing_state',lambda n:n['states'].pop()),('quota',lambda n:n.__setitem__('preparation_refusal_session_recovers',False)),('unwind',lambda n:n.__setitem__('unwind_restores_fields_and_retires_session',False)),('production',lambda n:n.__setitem__('production_enabled',True)),('type',lambda n:n.__setitem__('persistent_mesh_lists',1))]:
 n=copy.deepcopy(native);mutation(n)
 try:validate(source,n)
 except (AssertionError,KeyError):rejected.append(name)
 else:raise AssertionError('accepted '+name)
old=json.load(open(sys.argv[3]));
try:states(source['states'][:3],old['states'])
except AssertionError:old_rejected=True
else:raise AssertionError('literal old identity failure accepted')
print(json.dumps({'matched_states':4,'negative_controls':rejected,'literal_old_identity_rejected':old_rejected,'scope':'root append and shallow field failure restoration','full_scripting_acceptance':False}))
