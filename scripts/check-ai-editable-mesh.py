import json,sys,copy
GAPS=('face_loop_preserved','attrs_dict_preserved','vertex_position_preserved')
def validate(source,native):
 assert [s['name'] for s in source['states']]==['initial','shared','abort']
 assert [s['name'] for s in native['states']]==['initial','shared','abort']
 for a,b in zip(source['states'],native['states']):
  assert a.keys()==b.keys()
  for key in a:
   if a['name']=='abort' and key in GAPS:
    assert type(a[key])is bool and a[key]is False
    assert type(b[key])is bool and b[key]is True
   else: assert type(a[key])is type(b[key]) and a[key]==b[key],(a['name'],key)
 for key in ('persistent_mesh_lists','real_face_vertex_identity','source_welded_vertex_reuse','detached_face_valid_after_abort','native_projection','face_live_geometry_reads','two_mesh_publication','second_owner_pressure_retains_both','private_scope_edit_survives_error','final_zero_with_cycle'):
  assert type(native[key])is bool and native[key]is True,key
 for key in ('general_existing_object_rollback','undo_redo_controller','production_enabled'):
  assert type(native[key])is bool and native[key]is False,key
 assert source['face_live_geometry_reads']is True and source['private_scope_edit_survives_error']is True
source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native)
mutations=[]
for name,mutation in [
 ('position',lambda n:n['states'][1]['mesh']['vertices'][0].__setitem__(0,7)),
 ('missing_state',lambda n:n['states'].pop()),
 ('face_identity',lambda n:n['states'][0].__setitem__('held_face_live',False)),
 ('hidden_gap',lambda n:n['states'][2].__setitem__('attrs_dict_preserved',False)),
 ('pressure',lambda n:n.__setitem__('second_owner_pressure_retains_both',False)),
 ('production',lambda n:n.__setitem__('production_enabled',True)),
 ('type',lambda n:n.__setitem__('persistent_mesh_lists',1)),
]:
 n=copy.deepcopy(native);mutation(n)
 try:validate(source,n)
 except (AssertionError,KeyError):mutations.append(name)
 else:raise AssertionError('mutation accepted '+name)
if '--full' in sys.argv:
 assert source['states']==native['states'],'Required original wrapper replacement identity remains divergent'
print(json.dumps({'append_states':3,'negative_controls':mutations,'retained_required_identity_gaps':list(GAPS),'full_acceptance':False}))
