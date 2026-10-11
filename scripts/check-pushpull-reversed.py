import copy,json,sys
from pathlib import Path
names=['reversed-door','lone-sheet','front-recess','back-recess']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());remaining=json.loads(Path(sys.argv[3]).read_text())
def ring(p):return min(tuple(tuple(x)for x in p[i:]+p[:i])for i in range(len(p)))
def fields(fs):return sorted((ring(f['vertices']),tuple(sorted(ring(h)for h in f['holes'])),json.dumps(f['attrs'],sort_keys=True),f['interior'])for f in fs)
def check(n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in s['cases']]==names==[c['name']for c in n['cases']]
 for a,b in zip(s['cases'],n['cases'],strict=True):
  assert b['normal']==a['normal'] and b['distance']==a['distance']
  assert b['preview_cancel_exact']is True and b['history_exact']is True
  assert sorted(ring(f['vertices'])for f in a['preview'])==sorted(ring(f)for f in b['preview'])
  sourcecap=[f for f in a['result'] if f['attrs'].get('vendor')==71 and any(p not in a['source'][a['face']]['vertices']for p in f['vertices'])]
  nativecap=[f for f in b['result'] if f['attrs'].get('vendor')==71 and any(p not in a['source'][a['face']]['vertices']for p in f['vertices'])]
  assert len(sourcecap)==len(nativecap)==1 and fields(sourcecap)==fields(nativecap)
  if a['name']=='reversed-door':assert fields(a['result'])!=fields(b['result']) and b['result']==remaining
  else:assert fields(a['result'])==fields(b['result'])
check(n);rejected=[]
for kind in ['empty','duplicate','wrong_normal','wrong_preview','wrong_commit_direction','lost_metadata','lost_history','lost_support_vertex']:
 b=copy.deepcopy(n)
 if kind=='empty':b['cases']=[]
 elif kind=='duplicate':b['cases'][1]=b['cases'][0]
 elif kind=='wrong_normal':b['cases'][0]['normal']=[0,1,0]
 elif kind=='wrong_preview':b['cases'][0]['preview'][0][0][1]*=-1
 elif kind=='wrong_commit_direction':
  for f in b['cases'][0]['result']:
   if f['attrs'].get('vendor')==71:
    for p in f['vertices']:p[1]*=-1
 elif kind=='lost_metadata':b['cases'][0]['result'][2]['attrs'].pop('vendor')
 elif kind=='lost_history':b['cases'][0]['history_exact']=False
 else:b['cases'][2]['result'][0]['vertices'].pop()
 try:check(b)
 except AssertionError:rejected.append(kind)
 else:raise AssertionError('accepted '+kind)
report={'direction_preview_recipe_exact':4,'final_oriented_fields_exact':3,'negative_controls':rejected,'actual_app_history_cancel_exact':True,'native_visible_suppression_proved':False,'remaining':{'reversed-door':'Original rebuild modifies/paints floor and adds base face; native retains original support/floor (8 vs7 faces).','visible-preview':'Borrowed sweep descriptor is unconsumed by native Vulkan/Metal; source camera-aware suppression required.','closed-reversed-acquisition':'Original entry orientation and arbitrary Qt precision beyond these four open controls remain unproved.'}}
Path(sys.argv[4]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
