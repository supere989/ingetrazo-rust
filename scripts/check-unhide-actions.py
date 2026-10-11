#!/usr/bin/env python3
"""Strict action-state comparison; only selection set ordering is unordered."""
import copy,json,sys
from pathlib import Path
def validate(expected,actual):
 assert isinstance(actual,list) and len(actual)==len(expected),'state inventory'
 for source,native in zip(expected,actual):
  assert set(source)==set(native),'state fields'
  source=copy.deepcopy(source);native=copy.deepcopy(native)
  if isinstance(source.get('selected'),list):source['selected'].sort();native['selected'].sort()
  assert source==native,f"{source['label']}: {source} != {native}"
def main(base):
 base=Path(base)
 pairs=[('original.json','native.json'),('original-nested.json','native-nested.json')]
 states=0;negative=0
 for s,n in pairs:
  expected=json.loads((base/s).read_text());actual=json.loads((base/n).read_text());validate(expected,actual);states+=len(expected)
  for field in ['held_hidden','held_live','undo','redo','selected','edge_hidden','face_hidden','group_hidden','flags','depth']:
   row=next((i for i,r in enumerate(actual) if field in r),None)
   if row is None:continue
   bad=copy.deepcopy(actual);value=bad[row][field]
   if isinstance(value,list):
    if not value:bad[row][field]=[True]
    elif isinstance(value[0],bool):bad[row][field][0]=not value[0]
    elif isinstance(value[0],int):bad[row][field][0]+=1
    else:bad[row][field][0]=['corrupt']
   else:bad[row][field]+=1
   try:validate(expected,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('mutation accepted: '+field)
  for bad in [actual[:-1],list(reversed(actual)),actual+[actual[-1]]]:
   try:validate(expected,bad)
   except AssertionError:negative+=1
   else:raise AssertionError('inventory mutation accepted')
 print(json.dumps({'states':states,'negative_controls':negative,'result':'PASS'}))
if __name__=='__main__':main(sys.argv[1])
