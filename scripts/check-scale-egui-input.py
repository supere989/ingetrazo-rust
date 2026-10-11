#!/usr/bin/env python3
"""Exact bounded RawInput consumer and pinned Qt eye-clip comparisons."""
import copy,json,sys
NAMES=['threshold-7.75','threshold-8.25','threshold-9','coalesced-grab-move','click-move-click','drag-release','escape']
OLD=[[0.,0.,0.],[2.,0.,0.],[2.,3.,0.],[0.,3.,0.]]
NEW=[[0.,0.,0.],[4.,0.,0.],[4.,6.,0.],[0.,6.,0.]]
def validate(source,native):
 assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [x['name'] for x in source['cases']]==['crossing','reversed','behind','front']
 assert native['clip_cases']==[{'name':x['name'],'result':x['result']} for x in source['cases']]
 assert [x['name'] for x in native['cases']]==NAMES
 rows={x['name']:x for x in native['cases']}
 def state(name,phase,active,positions,undo):
  x=rows[name][phase];assert x['active'] is active;assert x['positions']==positions;assert x['undo']==undo
 state(NAMES[0],'press',True,OLD,0)
 for name in NAMES[1:3]:state(name,'press',False,OLD,0)
 state(NAMES[3],'press',True,NEW,0)
 for phase in ['press','release']:state(NAMES[4],phase,True,OLD,0)
 state(NAMES[4],'preview',True,NEW,0)
 for phase in ['commit','redo']:state(NAMES[4],phase,False,NEW,1)
 state(NAMES[4],'undo',False,OLD,0)
 state(NAMES[5],'preview',True,NEW,0);state(NAMES[5],'commit',False,NEW,1)
 state(NAMES[6],'cancel',False,OLD,0)
source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native)
mutations=[]
a=copy.deepcopy(native);a['cases']=[];mutations.append(a)
a=copy.deepcopy(native);a['cases'][0]['press']['positions'][1][0]+=2**-22;mutations.append(a)
a=copy.deepcopy(native);a['cases'][1]['press']['active']=True;mutations.append(a)
a=copy.deepcopy(native);a['cases'][3]['press']['active']=False;mutations.append(a)
a=copy.deepcopy(native);a['cases'][4]['undo']['positions']=NEW;mutations.append(a)
a=copy.deepcopy(native);a['clip_cases'][0]['result']=None;mutations.append(a)
for a in mutations:
 try:validate(source,a)
 except AssertionError:continue
 raise AssertionError('corruption accepted')
print(json.dumps({'passed':True,'consumer_cases':7,'exact_qt_clip_cases':4,'rejected_mutations':len(mutations),'scope':'actual egui RawInput, not OS/rendered capture'}))
