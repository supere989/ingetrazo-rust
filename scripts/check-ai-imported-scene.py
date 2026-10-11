import copy,json,sys
from pathlib import Path
def validate(s,n):
 assert len(s['states'])==len(n['states'])==4
 assert [x['name'] for x in s['states']]==['imported','edited','undone','redone']
 assert s['states']==n['states']
 assert s['production_enabled'] is n['production_enabled'] is False
 for key in ('actual_native_scene_import','packed_uids_preserved','native_document_edit_return'):assert n[key] is True
 assert n['final_charge']==0
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());validate(s,n);rejected=[]
for label,change in [('attrs',lambda v:v['states'][1]['faces'][0]['attrs'].__setitem__('mat','Wrong')),('undo',lambda v:v['states'][2].__setitem__('redo',0)),('id',lambda v:v.__setitem__('packed_uids_preserved',False)),('missing',lambda v:v['states'].pop()),('charge',lambda v:v.__setitem__('final_charge',1))]:
 m=copy.deepcopy(n);change(m)
 try:validate(s,m)
 except (AssertionError,KeyError,TypeError):rejected.append(label)
 else:raise AssertionError(label)
print(json.dumps({'states':4,'rejected':rejected,'production_enabled':False},indent=2))
