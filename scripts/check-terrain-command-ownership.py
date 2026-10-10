#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text());boolean=json.loads(Path(sys.argv[4]).read_text())
assert s['provenance']['original_python_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert s['cases']==n['cases']
assert old['cases']['ordinary']['states'][1]['terrain'] is True and n['cases']['ordinary']['states'][1]['terrain'] is False
assert boolean['cases']['terrain_owner_runtime_rebuild'][3]==[9.0,False] and n['cases']['terrain_owner_runtime_rebuild'][3] is None
for key in ['ordinary','folded']:
 bad=copy.deepcopy(n);bad['cases'][key]['states'][1]['terrain']=True
 assert bad['cases']!=s['cases']
for index in [3,5]:
 bad=copy.deepcopy(n);bad['cases']['terrain_owner_runtime_rebuild'][index]=[9.0,False]
 assert bad['cases']!=s['cases']
print(json.dumps({'exact_bounded_presentation_history':True,'exact_owned_terrain_states':6,'stale_visibility_negative':True,'literal_bool_only_redo_negative':True,'four_payload_negatives':True,'full_P15':False}))
