#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
assert s['provenance']['original_python_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
a=s['cases'];b=n['cases'];assert a['import_object_lifetime']==b['import_object_lifetime']
for name in ['ordinary','folded']:
 assert a[name]['entry']==b[name]['entry']
 assert a[name]['history_before_finish']==b[name]['history_before_finish']
 assert len(a[name]['states'])==len(b[name]['states'])
 for expected,actual in zip(a[name]['states'],b[name]['states']):
  for key in ['views','path','sheets']:assert expected[key]==actual[key],(name,key,expected,actual)
assert a['folded']['states']==b['folded']['states']
assert old['cases']['ordinary']['history_before_finish']==4!=b['ordinary']['history_before_finish']
for field in ['views','path','sheets']:
 bad=copy.deepcopy(b);bad['ordinary']['states'][1][field]=[]
 assert bad['ordinary']['states'][1][field]!=a['ordinary']['states'][1][field]
bad=copy.deepcopy(b);bad['import_object_lifetime']['states'][-1].reverse();assert bad['import_object_lifetime']!=a['import_object_lifetime']
print(json.dumps({'saved_view_path_sheet_history':True,'exact_import_lifetime_states':4,'old_extra_history_negative':True,'four_payload_negatives':True,'terrain_ordinary_parity':a['ordinary']['states']==b['ordinary']['states'],'full_presentation_parity':False}))
