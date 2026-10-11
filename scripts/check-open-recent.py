#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);s=json.loads((p/'original-recent.json').read_text());n=json.loads((p/'native-recent.json').read_text())
phases=['cap','alias','import','none','missing-filter-retains','clear']
assert [x['phase']for x in s['rows']]==phases
assert s['rows'][0]['recent']==[f'doc-{i}.igz'for i in range(11,1,-1)]
assert s['rows'][1]['recent']==['doc-5.igz']+[f'doc-{i}.igz'for i in range(11,1,-1)if i!=5]
assert s['missing']==['warning'] and s['cancel']==['finish','prompt'] and s['success']==['finish','prompt','open:doc-11.igz']
assert 'doc-5.igz'not in s['filtered'] and 'doc-5.igz'in s['rows'][4]['recent']
assert s['restart']==[] and s['empty']==[{'label':'(no recent documents)','enabled':False}]
for i,row in enumerate(s['menu'][:10],1):
 assert row['label'].startswith(f'&{i}  ') and row['tooltip'].endswith(s['rows'][1]['recent'][i-1]) and row['enabled']
 assert Path(row['tooltip']).is_absolute()
 assert row['label']==f"&{i}  {Path(row['tooltip']).name}  —  {Path(row['tooltip']).parent}"
def validate(x):
 assert type(x)is dict and x.keys()=={'rows','restart','app_preservation'}
 assert type(x['app_preservation'])is bool and x['app_preservation']
 assert type(x['restart'])is list and x['restart']==[]
 assert x['rows']==s['rows']
validate(n)
negatives=[]
for kind in ['import','duplicate','order','missing','clear','restart','document']:
 b=copy.deepcopy(n)
 if kind=='import':b['rows'][2]['recent'].insert(0,'model.obj')
 elif kind=='duplicate':b['rows'][1]['recent'].append('doc-5.igz')
 elif kind=='order':b['rows'][0]['recent'].reverse()
 elif kind=='missing':b['rows'][4]['recent'].remove('doc-5.igz')
 elif kind=='clear':b['rows'][-1]['recent']=['doc-11.igz']
 elif kind=='restart':b['restart']=['doc-11.igz']
 else:b['app_preservation']=False
 try:validate(b)
 except AssertionError:negatives.append(kind)
 else:raise AssertionError(kind)
print(json.dumps(dict(source_phases=6,negatives=negatives,exact=True)))
