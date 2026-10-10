#!/usr/bin/env python3
import json,copy
from pathlib import Path
root=Path(__file__).resolve().parents[1];d=root/'docs/audits/ai-degenerate-faces'
cases=json.loads((root/'tools/ai-interpreter-proof/face-degenerate-cases.json').read_text());names=[name for c in cases for name in [c['name'],c['name']+'-undo',c['name']+'-redo']]
def validate(native,original):
 for value in [native,original]:
  if not isinstance(value,list)or any(not isinstance(s,dict)for s in value)or[s.get('name')for s in value]!=names:raise ValueError('Degenerate inventory incomplete')
 if native!=original:raise ValueError('Adjacency/radial/history differs')
 return True
if __name__=='__main__':
 n=json.loads((d/'native.json').read_text());o=json.loads((d/'original.json').read_text());validate(n,o);mutations=[]
 a=copy.deepcopy(n);a[3]['adjacency'][0].append(0);mutations.append(a)
 a=copy.deepcopy(n);a[6]['edges'][0]['faces']=[0];mutations.append(a)
 a=copy.deepcopy(n);a[9]['faces'][0]['outer'].pop();mutations.append(a)
 a=copy.deepcopy(n);a[4]['vertices'].append([0,0,0]);mutations.append(a)
 a=copy.deepcopy(n);a[0]['undo']=0;mutations.append(a)
 mutations.extend([n[:-1],{}]);a=copy.deepcopy(n);a[0]=None;mutations.append(a)
 for a in mutations:
  try:validate(a,o)
  except ValueError:continue
  raise SystemExit('Negative accepted')
 if json.loads((d/'proper-regression.json').read_text())!=json.loads((root/'docs/audits/ai-face-transaction/original.json').read_text()):raise SystemExit('Proper-ring regression differs')
 print('PASS:27 source states; eight rejected mutations;18 proper-ring source regression states')
