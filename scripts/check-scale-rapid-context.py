#!/usr/bin/env python3
"""Strict nine-recipe handler/egui Scale comparison; no pixel/OS claim."""
import copy,json,sys
from pathlib import Path
NAMES=('ctrl-tap','ctrl-double','shift-tap','shift-double','ctrl-shift','ctrl-shift-twice','image-finish-preview-commit','image-exit-two','image-commit-finish-retype')
FIELDS=('positions','images','depth','active','undo','redo')
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def compare(source,native):
 assert source['source_pin']==PIN
 assert tuple(c['name'] for c in source['cases'])==NAMES
 assert tuple(c['name'] for c in native['cases'])==NAMES
 assert native['expired_head_preserved'] is True
 counts=0
 for a,b in zip(source['cases'],native['cases']):
  assert a['kind']==b['kind']
  expected={'before-taps','after-taps','commit','undo','redo'} if a['kind']=='rapid' else {'preview','boundary','retype','undo','redo','tool-cancel',('commit-before' if a['name']=='image-commit-finish-retype' else 'commit')}
  if a['name']=='image-exit-two':expected.add('outer-boundary')
  assert set(a['states'])==set(b['states'])==expected
  accepted=a['accepted'] if isinstance(a['accepted'],list) else [a['accepted']]
  assert accepted==b['accepted'] and all(x is True for x in accepted)
  for phase,state in a['states'].items():
   for key in FIELDS:
    assert state[key]==b['states'][phase][key],(a['name'],phase,key,state[key],b['states'][phase][key])
   counts+=1
 return counts
def rejected(s,n):
 try:compare(s,n)
 except (AssertionError,KeyError,TypeError):return True
 return False
if __name__=='__main__':
 root=Path(sys.argv[1]);s=json.loads((root/'source.json').read_text());n=json.loads((root/'native.json').read_text());count=compare(s,n)
 controls={}
 for name,mutate in [('empty',lambda x:x.update(cases=[])),('truncated',lambda x:x['cases'].pop()),('duplicated',lambda x:x['cases'].__setitem__(1,copy.deepcopy(x['cases'][0]))),('lost-retype',lambda x:x['cases'][-1]['states']['retype'].__setitem__('images',x['cases'][-1]['states']['commit-before']['images'])),('history-drift',lambda x:x['cases'][-1]['states']['retype'].__setitem__('undo',99)),('revived-expired',lambda x:x.update(expired_head_preserved=False))]:
  bad=copy.deepcopy(n);mutate(bad);controls[name]=rejected(s,bad);assert controls[name]
 controls['actual-final-state-only-ingress']=rejected(s,json.loads((root/'final-only.json').read_text()));assert controls['actual-final-state-only-ingress']
 initial=json.loads((root/'initial-failure.json').read_text());initial['expired_head_preserved']=True;controls['retained-initial-context-failure']=rejected(s,initial);assert controls['retained-initial-context-failure']
 print(json.dumps({'cases':len(NAMES),'exact_states':count,'fields':FIELDS,'negative_controls':controls,'scope':'Qt delivered modifier key handlers vs native timeline/egui; actual app context/value/history handlers, no physical input/pixel claim'},indent=2))
