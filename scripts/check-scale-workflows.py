#!/usr/bin/env python3
"""Literal app/source Scale states plus an explicit source registry divergence."""
import copy,json,sys
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text());warp=json.loads(Path(sys.argv[2]).read_text());native=json.loads(Path(sys.argv[3]).read_text())
NAMES=['loose','image','mixed','baked','baked-rotated','placed','reflected']
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def check(n):
 assert source['source_pin']==warp['source_pin']==PIN
 assert [c['kind']for c in source['cases']]==[c['kind']for c in n['cases']]==NAMES
 exact=0
 for s,c in zip(source['cases'],n['cases'],strict=True):
  assert c['accepted']==s['accepted']==[True,True]
  assert c['states'].keys()==s['states'].keys()
  for stage,a in s['states'].items():
   b=c['states'][stage]
   if s['kind']=='loose' and stage=='cancel-preview':
    # Source single-pass mirrored registry lost two keys, confirmed by the
    # retained diagnostic: selection is live but _grab reacquires vertices2/3.
    # Native must transform ALL four original owners about the held centre.
    expected=copy.deepcopy(s['states']['redo'])
    points=[[2.5,3.75,0.],[-.5,3.75,0.],[-.5,-.75,0.],[2.5,-.75,0.]]
    expected['mesh']['vertices']=points;expected['mesh']['faces'][0]['vertices']=points
    assert b==expected and b!=a
   else:assert a==b,(s['kind'],stage);exact+=1
 assert n['warp']['accepted']==warp['accepted']==[True,True]
 assert n['warp']['states']==warp['states'];exact+=len(warp['states'])
 assert exact==74
check(native);negatives=[]
for kind in ['roster','preview','cancel','undo','redo','retype-history','group-axes','image-frame','mirror-lookup','warp-topology']:
 bad=copy.deepcopy(native);c=bad['cases'][0]['states']
 if kind=='roster':bad['cases'].pop()
 elif kind=='preview':c['preview']['mesh']['vertices'][0][0]+=1
 elif kind=='cancel':c['cancel']['mesh']['vertices'][0][0]+=1
 elif kind=='undo':c['undo']['mesh']['faces'].pop()
 elif kind=='redo':c['redo']['mesh']['faces'].pop()
 elif kind=='retype-history':c['retype']['undo']=2
 elif kind=='group-axes':bad['cases'][3]['states']['commit']['groups'][0]['axes']=None
 elif kind=='image-frame':bad['cases'][1]['states']['preview']['images'][0][1][0]+=1
 elif kind=='mirror-lookup':c['cancel-preview']=copy.deepcopy(source['cases'][0]['states']['cancel-preview'])
 else:bad['warp']['states']['commit']['mesh']['faces'].pop()
 try:check(bad)
 except (AssertionError,KeyError,IndexError):negatives.append(kind)
 else:raise AssertionError('Accepted corruption '+kind)
report={'exact_literal_states':74,'workflows':8,'native_registry_integrity_divergence':1,'negative_controls':negatives,'scope':'Actual original ScaleTool/History versus native app dispatch; semantic controller/model/history only. Rendered grip pixels, real OS input, shared-prototype app workflows and complete context lifetime remain open.'}
Path(sys.argv[4]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
