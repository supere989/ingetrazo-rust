#!/usr/bin/env python3
import json,copy,sys
from pathlib import Path
MOVE={'selected','implicit','locked_implicit','locked_selected','hidden_selected','cancel','copy','fractional','fractional_refusal'}
MOD={f'{kind}_{state}'for kind in ['Face','Edge','Group']for state in ['selected','unselected']}
def read(p):
 d=json.loads(Path(p).read_text());return d.get('cases',d)
def check(a,b,registry):
 if set(a)!=registry or set(b)!=registry:raise AssertionError('Missing/unexpected mandatory cases')
 if a!=b:raise AssertionError('Mismatch: '+','.join(k for k in sorted(registry)if a[k]!=b[k]))
def main():
 a,b,c,d=[read(p)for p in sys.argv[1:5]];check(a,b,MOVE);check(c,d,MOD)
 for key in ['selected','fractional','cancel','copy']:
  bad=copy.deepcopy(b);bad[key][-1]['origin'][0]+=1
  try:check(a,bad,MOVE)
  except AssertionError:pass
  else:raise AssertionError('Frame mutation accepted')
 try:check(a,{},MOVE)
 except AssertionError:pass
 else:raise AssertionError('Empty accepted')
 print('PASS nine operative Move families and six geometry remove-modifier controls; five negatives')
if __name__=='__main__':main()
