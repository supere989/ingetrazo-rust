#!/usr/bin/env python3
"""Strict operative picking comparison; literal case registry is mandatory."""
import copy,json,sys
from pathlib import Path
required={q['name'] for q in json.loads((Path(__file__).parent/'image-plane-pick-input.json').read_text())}
def check(source,native):
 a,b=source.get('cases',source),native.get('cases',native)
 if set(a)!=required or set(b)!=required: raise AssertionError('Missing/unexpected mandatory cases')
 if a!=b: raise AssertionError('Picking differs: '+','.join(k for k in sorted(required) if a[k]!=b[k]))
def main():
 a,b=[json.loads(Path(p).read_text()) for p in sys.argv[1:3]]
 b={'cases':b.get('cases',b)}
 check(a,b)
 for key,field in [('bare','selectable'),('own_locked','locked_context'),('bare','selected'),('vertical','workplane')]:
  if key not in b['cases']: continue
  bad=copy.deepcopy(b);bad['cases'][key][field]='negative'
  try:check(a,bad)
  except AssertionError:pass
  else:raise AssertionError('Mutation accepted')
 try:check(a,{'cases':{}})
 except AssertionError:pass
 else:raise AssertionError('Empty corpus accepted')
 print(f'PASS {len(required)} literal original/native picking cases; mutation and empty controls rejected')
if __name__=='__main__':main()
