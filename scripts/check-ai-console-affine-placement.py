import copy,json,sys
from pathlib import Path
def check(original,native):
 assert len(original['states'])==len(native['states'])==11
 assert original['states']==native['states'],'immediate typed placement/history mismatch'
 assert native['refusals']==[True,True,True],'atomic refusal inventory'
original=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());check(original,native)
for kind in ['position','reflection','aliases','undo','missing','refusal']:
 changed=copy.deepcopy(native)
 if kind=='position':changed['states'][1]['transforms'][0][12]+=.125
 if kind=='reflection':changed['states'][2]['transforms'][0][0]*=-1
 if kind=='aliases':changed['states'][3]['aliases'][0][1]=False
 if kind=='undo':changed['states'][7]['undo']+=1
 if kind=='missing':changed['states'].pop()
 if kind=='refusal':changed['refusals'][0]=False
 try:check(original,changed)
 except AssertionError:pass
 else:raise AssertionError('accepted negative '+kind)
print('11 exact immediate affine app states + 3 real atomic refusals + 6 rejecting controls PASS')
