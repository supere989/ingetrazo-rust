#!/usr/bin/env python3
"""Exact direct-kernel delta acceptance; retain source sliver self-edge gap."""
import sys,json,copy
NAMES=['cross-positive','cross-negative','distance-reject','tie-and-order','neighbor-first','radial-2','radial-3','hole-split-weld']+['bulk-seed-'+str(i) for i in range(8)]+['large-grid-1200','same-cell-diagonal','place-collision-owner','exact-half-diagonal']
COUNTS=[2,2,2,4,3,5,6,3]+[1]*9+[2,4,2]
def equal(a,b):
 if type(a)is not type(b):return False
 if isinstance(a,dict):return a.keys()==b.keys()and all(equal(a[k],b[k])for k in a)
 if isinstance(a,list):return len(a)==len(b)and all(equal(x,y)for x,y in zip(a,b))
 return a==b

def validate(source,native):
 assert isinstance(source,list)and isinstance(native,list)
 assert [c['name']for c in source]==NAMES==[c['name']for c in native], 'mandatory case inventory'
 result=[]
 for s,n,count in zip(source,native,COUNTS):
  assert len(s['states'])==len(n['states'])==count,'mandatory state inventory'
  match=equal(s,n)
  bulk=s['name'].startswith(('bulk-','large-'))
  if not bulk:assert match,s['name']+' literal mismatch'
  else:
   a=s['states'][0];b=n['states'][0]
   assert equal(a['vertices'],b['vertices'])and equal(a['faces'],b['faces']),s['name']+' changed vertices/faces'
   missing=[e for e in a['edges']if e['v'][0]==e['v'][1]]
   assert len(missing)==1 and missing[0]['faces']==[len(a['faces'])-1]*3,'source sliver law'
   assert equal(a['edges'],b['edges'])or equal([e for e in a['edges']if e['v'][0]!=e['v'][1]],b['edges']),s['name']+' lost proper edge payload/order'
  result.append({'name':s['name'],'raw_equal':match,'states':count,'source_vertices':len(s['states'][-1]['vertices']),'source_edges':len(s['states'][-1]['edges']),'native_edges':len(n['states'][-1]['edges']),'remaining_gap':None if match else 'welded sliver source self-edge and triple radial incidence omitted'})
 return result

def main():
 source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));cases=validate(source,native);controls={}
 def reject(name,change):
  x=copy.deepcopy(native);change(x)
  try:validate(source,x)
  except (AssertionError,KeyError,IndexError,TypeError):controls[name]=True;return
  raise AssertionError('negative accepted '+name)
 reject('empty',lambda x:x.clear())
 reject('duplicate-case',lambda x:x.__setitem__(1,copy.deepcopy(x[0])))
 reject('missing-phase',lambda x:x[5]['states'].pop())
 reject('wrong-weld',lambda x:x[0]['states'][1]['vertices'].append([0.000051,0.,0.]))
 reject('wrong-radial-order',lambda x:x[6]['states'][-1]['edges'][-1]['faces'].reverse())
 reject('wrong-welded-subedge-metadata',lambda x:x[7]['states'][-1]['edges'][-2].__setitem__('soft',False))
 reject('lost-large-edge',lambda x:x[16]['states'][0]['edges'].pop(0))
 reject('wrong-painted-face',lambda x:x[5]['states'][-1]['faces'][0]['attrs'].__setitem__('vendor',{'wall':99}))
 reject('boolean-id',lambda x:x[5]['states'][-1]['faces'][0]['loop'].__setitem__(0,False))
 reject('wrong-half-tie-weld',lambda x:x[19]['states'][-1]['vertices'].pop())
 return {'delta_passed':True,'p05_complete':False,'raw_equal_cases':sum(c['raw_equal']for c in cases),'case_count':len(cases),'cases':cases,'negative_controls':controls,'boundary':'No normalization of full source sliver mismatch; native f64/integrity guards execute in named binary.'}
if __name__=='__main__':print(json.dumps(main(),indent=2))
