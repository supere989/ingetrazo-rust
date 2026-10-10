#!/usr/bin/env python3
import json,sys
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
assert s['provenance']['original_python_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
a=s['cases'];b=n['cases']
assert a['folded']['entry']==b['folded']['entry'] and a['folded']['states']==b['folded']['states']
assert a['ordinary']['entry']==b['ordinary']['entry'] and a['ordinary']['states'][0]==b['ordinary']['states'][0]
assert a['ordinary']['history_before_finish']==2 and b['ordinary']['history_before_finish']==4
assert a['ordinary']['states'][1]['views']==[['Scene 1',33.0]] and b['ordinary']['states'][1]['views']==[['Scene 1',20.0]]
assert a['ordinary']['states'][1]['terrain'] is False and b['ordinary']['states'][1]['terrain'] is True
assert a['ordinary']['states'][1]['path']!=b['ordinary']['states'][1]['path']
print(json.dumps({'finish_folded_boundary_pass':True,'ordinary_history_parity':False,'retained_expected_failure':True}))
