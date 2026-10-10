#!/usr/bin/env python3
import json,copy
from pathlib import Path
root=Path(__file__).resolve().parents[1];d=root/'docs/audits/ai-face-transaction'
names=['empty','triangle','shared','shared-undo','shared-redo','hole','hole-undo','hole-redo','explicit-error-rollback','slanted','slanted-undo','slanted-redo','reflected','reflected-undo','reflected-redo','extrude-geometry','extrude-undo','extrude-redo']
def validate(native,original):
 if not isinstance(original,list) or any(not isinstance(s,dict)for s in original) or [s.get('name')for s in original]!=names:raise ValueError('Original inventory incomplete')
 if not isinstance(native,list) or any(not isinstance(s,dict)for s in native) or [s.get('name')for s in native]!=names:raise ValueError('Native inventory incomplete')
 if native!=original:raise ValueError('Face geometry/radial/history differs')
 return True
if __name__=='__main__':
 n=json.loads((d/'native.json').read_text());o=json.loads((d/'original.json').read_text());validate(n,o)
 mutations=[]
 a=copy.deepcopy(n);a[2]['edges'][0]['faces']=[0];mutations.append(a)
 a=copy.deepcopy(n);a[5]['faces'][-1]['holes']=[];mutations.append(a)
 a=copy.deepcopy(n);a[8]['vertices'].append([30,0,0]);mutations.append(a)
 a=copy.deepcopy(n);a[9]['faces'][-1]['outer'].reverse();mutations.append(a)
 a=copy.deepcopy(n);a[15]['edges'][0]['vertices'].reverse();mutations.append(a)
 a=copy.deepcopy(n);a[0]=42;mutations.append(a)
 mutations.extend([n[:-1],{}])
 for a in mutations:
  try:validate(a,o)
  except ValueError:continue
  raise SystemExit('Negative control accepted')
 e=json.loads((d/'edge-history-regression.json').read_text());w=json.loads((d/'weld-regression.json').read_text())
 if e!=json.loads((root/'docs/audits/ai-private-owner/original.json').read_text())or w!=json.loads((root/'docs/audits/ai-weld-boundary/original.json').read_text()):raise SystemExit('Existing source regression failed')
 print('PASS: 18 operative source/native mesh states; eight rejected mutations; 12 edge-history and seven weld regressions')
