#!/usr/bin/env python3
"""Exact original app states and actual Python reopen; mutation checks reuse validators."""
import copy,gzip,json,sys
from pathlib import Path
CASES=['classic','shared','nested','oblique','context']
PHASES=['before','invalid','changed','undo','redo','cancel']
def exact(a,b):
 if isinstance(a,bool)or isinstance(b,bool):return type(a)==type(b)and a==b
 if isinstance(a,dict):return isinstance(b,dict)and a.keys()==b.keys()and all(exact(a[k],b[k])for k in a)
 if isinstance(a,list):return isinstance(b,list)and len(a)==len(b)and all(exact(x,y)for x,y in zip(a,b))
 return a==b

def validate(source,native,cases=CASES):
 roster=[(c,p)for c in cases for p in PHASES+(['finish','finish-undo','finish-redo']if c=='context'else [])]
 assert [(x.get('case'),x.get('phase'))for x in source]==roster,'source inventory'
 assert [(x.get('case'),x.get('phase'))for x in native]==roster,'native inventory'
 assert exact(source,native),'literal application state mismatch'

def reopen_validate(source,reopened,cases=CASES):
 expected={c:next(x['groups']for x in source if x['case']==c and x['phase']==('finish'if c=='context'else 'changed'))for c in cases}
 assert exact(expected,reopened),'actual Python reopen mismatch'

def ids(value,out):
 if isinstance(value,dict):
  if 'uid'in value and 'name'in value:out.setdefault(value['name'],set()).add(value['uid'])
  for v in value.values():ids(v,out)
 elif isinstance(value,list):
  for v in value:ids(v,out)

def main():
 d=Path(sys.argv[1]);source=json.loads((d/'original.json').read_text());native=json.loads((d/'native.json').read_text());reopened=json.loads((d/'reopened.json').read_text());validate(source,native);reopen_validate(source,reopened)
 for c in CASES:
  a={};b={};ids(json.loads((d/(c+'.input.igz')).read_text()),a);ids(json.loads((d/(c+'.native.igz')).read_text()),b)
  assert all(b.get(k)==v for k,v in a.items()),'authored literal UID lost'
 rejected=0
 for mutate in [lambda v:v.pop(),lambda v:v[2]['groups'][0]['axes'].__setitem__(12,999),lambda v:v[8]['groups'][1]['xform'].__setitem__(12,999),lambda v:v[14]['groups'][0]['ext'].pop('opaque'),lambda v:v[2].__setitem__('undo',0),lambda v:v[31]['groups'][2]['xform'].__setitem__(0,999),lambda v:v[2]['selection'].clear()]:
  v=copy.deepcopy(native);mutate(v)
  try:validate(source,v)
  except AssertionError:rejected+=1
  else:raise AssertionError('mutation accepted')
 v=copy.deepcopy(reopened);v['classic'][0]['axes']=None
 try:reopen_validate(source,v)
 except AssertionError:rejected+=1
 else:raise AssertionError('lost axes accepted')
 assert rejected==8
 failures=Path(__file__).resolve().parents[1]/'docs/audits/change-axes'
 for name,check in [('initial-native-math-failure.json.gz',lambda old:validate(source[:24],old,CASES[:4])),('initial-classic-save-loss.json.gz',lambda old:reopen_validate(source[:24],old,CASES[:4])),('initial-context-finish-history-failure.json.gz',lambda old:validate(source,old))]:
  old=json.loads(gzip.decompress((failures/name).read_bytes()))
  try:check(old)
  except AssertionError:pass
  else:raise AssertionError('retained failure accepted: '+name)
 print(json.dumps({'retained_failures_rejected':3,'states':len(source),'actual_python_reopens':len(CASES),'rejecting_controls':rejected,'literal_uid_cases':len(CASES),'exact':True}))
if __name__=='__main__':main()
