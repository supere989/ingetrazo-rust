"""Strict original/current app nested ownership comparison; every negative uses validate."""
import copy,json,math,sys
NAMES=['import','nested-pose','detach','reinsert','shared-edit','membership-add','undo','redo','delete-parent','reinsert-parent','nested-create','children-replace']
def equal(a,b):
 assert type(a) is type(b),(type(a),type(b))
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a:equal(a[k],b[k])
 elif isinstance(a,list):
  assert len(a)==len(b)
  for x,y in zip(a,b):equal(x,y)
 else:assert a==b,(a,b)
def validate(source,native):
 assert set(source)=={'states'} and set(native)=={'states','refusals'}
 assert [s['name'] for s in source['states']]==NAMES
 assert native['refusals']==[True]*3 and all(type(v)is bool for v in native['refusals'])
 for state in source['states']:
  assert set(state)=={'name','rows','aliases','undo','redo'}
  count=len(state['rows']);assert count>=3
  assert len(state['aliases'])==count
  for i,row in enumerate(state['aliases']):
   assert len(row)==count and row[i] is True and all(type(v)is bool for v in row)
  for row in state['rows']:
   assert set(row)=={'path','name','matrix','effective','component','vertices'}
   assert row['path'] and all(type(i)is int and i>=0 for i in row['path'])
   for key in ['matrix','effective']:
    assert len(row[key])==16 and all(type(v)is float and math.isfinite(v) for v in row[key])
  assert type(state['undo'])is int and type(state['redo'])is int
 equal(source['states'],native['states'])
def main():
 source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native)
 def rejected(mut):
  candidate=copy.deepcopy(native);mut(candidate)
  try:validate(source,candidate)
  except (AssertionError,KeyError,TypeError,IndexError):return
  raise AssertionError('checker accepted altered nested ownership')
 rejected(lambda c:c['states'][0]['rows'].pop())
 rejected(lambda c:c['states'][0]['rows'][2]['matrix'].__setitem__(12,9.0))
 rejected(lambda c:c['states'][1]['rows'][2]['effective'].__setitem__(13,9.0))
 rejected(lambda c:c['states'][0]['aliases'][2].__setitem__(3,not c['states'][0]['aliases'][2][3]))
 rejected(lambda c:c['states'][4]['rows'][2]['vertices'][0].__setitem__(2,9.0))
 rejected(lambda c:c['states'][7].__setitem__('redo',99))
 rejected(lambda c:c['states'][0]['rows'][2].__setitem__('component',1))
 rejected(lambda c:c['refusals'].__setitem__(0,False))
 print('PASS 12 exact current app states, 3 refusal controls, 8 rejecting mutations')
if __name__=='__main__':main()
