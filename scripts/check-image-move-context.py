#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
REQUIRED={f'{kind}:{action}'for kind in ['plain','component','folded','nested','nested_folded']for action in ['finish_live','cancel_then_finish']}
def read(p):
 d=json.loads(Path(p).read_text());return d.get('cases',d)
def check(a,b):
 if set(a)!=REQUIRED or set(b)!=REQUIRED:raise AssertionError('Missing mandatory context cases')
 if a!=b:raise AssertionError('Mismatch: '+','.join(k for k in sorted(REQUIRED)if a[k]!=b[k]))
def main():
 a,b,r,s,old,oldroots=[read(p)for p in sys.argv[1:7]];check(a,b)
 if r!={'detached_tool_owner':True,'released_after_cancel':True}or s!=r:raise AssertionError('Tool-held metadata lifetime mismatch')
 failures={k for k in REQUIRED if a[k]!=old[k]}
 if failures!={f'{kind}:finish_live'for kind in ['plain','component','folded','nested','nested_folded']}:raise AssertionError('Literal prior context failure missing')
 if oldroots.get('detached_tool_owner')is not False:raise AssertionError('Literal prior metadata failure missing')
 for field,value in [('origin',[0,0,0]),('active',False),('undo',999)]:
  bad=copy.deepcopy(b);bad['plain:finish_live'][2][field]=value
  try:check(a,bad)
  except AssertionError:pass
  else:raise AssertionError('Mutation accepted')
 try:check(a,{})
 except AssertionError:pass
 else:raise AssertionError('Empty accepted')
 print(f'PASS {sum(len(v)for v in a.values())} states/ten source contexts, two lifetime controls, literal prior negatives and four mutations')
if __name__=='__main__':main()
