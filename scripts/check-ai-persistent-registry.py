#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
root=Path(__file__).resolve().parents[1]
def validate(weld,rows):
 original=json.loads((root/'docs/audits/ai-weld-boundary/original.json').read_text())
 if weld!=original: raise ValueError('Registry-backed source weld differs')
 if not isinstance(rows,list) or len(rows)!=3: raise ValueError('Missing base-size inventory')
 for row,n in zip(rows,[1,1000,50000]):
  expected={'base_vertices':n,'begins':1000,'allocation_calls':0,'allocated_bytes':0,'cold_vertex_rows':n,'lookup_calls':0,'varying_operations':1000 if n>1 else 0,'varying_lookup_calls':3000 if n>1 else 0}
  if row!=expected:raise ValueError('Allocation/lookup inventory differs')
 return True
if __name__=='__main__':
 d=root/'docs/audits/ai-persistent-registry';w=json.loads((d/'native-weld.json').read_text());r=json.loads((d/'begin-allocation.json').read_text());validate(w,r)
 mutations=[]
 a=copy.deepcopy(w);a[0]['edges'][0][0]=999;mutations.append((a,r))
 a=copy.deepcopy(r);a[2]['allocation_calls']=1;mutations.append((w,a))
 a=copy.deepcopy(r);a[2]['cold_vertex_rows']=50001;mutations.append((w,a))
 a=copy.deepcopy(r);a[2]['varying_lookup_calls']=3001;mutations.append((w,a))
 mutations.append((w,r[:-1]));mutations.append((w,{}))
 for a,b in mutations:
  try:validate(a,b)
  except ValueError:continue
  raise SystemExit('Negative control accepted')
 print('PASS: seven registry-backed source weld cases, three allocation inventories, six rejected mutations')
