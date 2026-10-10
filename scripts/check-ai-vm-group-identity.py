#!/usr/bin/env python3
"""Strict frozen original/native Group prerequisite checker, with rejection controls."""
import argparse,copy,json
from pathlib import Path
FLAGS=('held_mutation_visible_after_redo','persistent_mesh_faces_list_identity','per_face_color_conversion','helper_globals_not_user_shadowed','arbitrary_truthy_name_identity','outline_double_iteration','cross_session_name_counter','caught_python_helper_error_preserves_prior_group')
NATIVE=('caught_admission_discards','refusal_preserves_redo','shared_budget_refusal_preserves_history_and_charges','detached_group_mesh_face_identity','external_handle_retains_mesh_and_charge','other_controller_scope_survives_teardown','cyclic_payload_ledger_released','ledger_released')
OPEN=('full_mesh_api','all_helper_orientation','existing_object_rollback','mutable_mesh_projection','production_enabled')
def exact(a,b,path):
    assert type(a) is type(b),f'{path}: type mismatch'
    if isinstance(b,dict):
        assert a.keys()==b.keys(),f'{path}: keys'
        for k in b:exact(a[k],b[k],path+'.'+k)
    elif isinstance(b,list):
        assert len(a)==len(b),f'{path}: length'
        for i,(x,y) in enumerate(zip(a,b)):exact(x,y,f'{path}[{i}]')
    else:assert a==b,f'{path}: value'
def validate(native,source):
    assert set(source)=={'group_states','geometry',*FLAGS},'Malformed source inventory'
    assert [s['name']for s in source['group_states']]==['finish-group','finish-undo','finish-redo'],'Missing original identity state'
    assert [s['name']for s in source['geometry']]==['ccw','cw','negative-height','flat','empty'],'Missing original geometry recipe'
    assert set(native)=={'group_states','geometry',*FLAGS,*NATIVE,*OPEN},'Malformed native inventory'
    exact(native['group_states'],source['group_states'],'group_states')
    exact(native['geometry'],source['geometry'],'geometry')
    for key in FLAGS:assert source[key] is True and native[key] is True,key
    for key in NATIVE:assert native[key] is True,key
    for key in OPEN:assert native[key] is False,key
    return True
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args()
    source=json.loads(a.source.read_text());native=json.loads(a.native.read_text());validate(native,source)
    mutations=[]
    for key in ('returned_is_placement','held_mesh_is_group','held_face_is_mesh_first'):
        n=copy.deepcopy(native);n['group_states'][2][key]=False;mutations.append((n,source))
    n=copy.deepcopy(native);n['geometry'][3]['mesh']['faces'][-1]['vertices'].reverse();mutations.append((n,source))
    for key in ('persistent_mesh_faces_list_identity','external_handle_retains_mesh_and_charge','other_controller_scope_survives_teardown','ledger_released'):
        n=copy.deepcopy(native);n[key]=False;mutations.append((n,source))
    n=copy.deepcopy(native);n['production_enabled']=True;mutations.append((n,source))
    s=copy.deepcopy(source);s['geometry'].pop();mutations.append((native,s))
    s=copy.deepcopy(source);s['group_states'].pop();mutations.append((native,s))
    n=copy.deepcopy(native);n['ledger_released']=1;mutations.append((n,source))
    for n,s in mutations:
        try:validate(n,s)
        except AssertionError:pass
        else:raise AssertionError('Negative control accepted')
    print(json.dumps({'identity_states':3,'geometry_recipes':5,'negative_controls_rejected':len(mutations),'production_enabled':False}))
