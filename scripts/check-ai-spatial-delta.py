#!/usr/bin/env python3
import json,copy
from pathlib import Path
root=Path(__file__).resolve().parents[1]
def validate(h,w,s):
 if h!=json.loads((root/'docs/audits/ai-private-owner/original.json').read_text()):raise ValueError('History differs')
 if w!=json.loads((root/'docs/audits/ai-weld-boundary/original.json').read_text()):raise ValueError('Registry weld differs')
 if not isinstance(s,list) or len(s)!=2:raise ValueError('Scaling inventory absent')
 for row,n in zip(s,[1000,50000]):
  if row['base_vertices']!=n or row['cold_vertex_rows']!=n or row['append_undo_redo']!=64 or row['publication_rows']!=388:raise ValueError('Operation inventory differs')
  if not 0<row['largest_allocation']<16384 or row['initial_capacity_growth_bytes']<=0:raise ValueError('Invalid growth scope')
 for key in ['allocation_calls','allocated_bytes','largest_allocation']:
  if s[0][key]!=s[1][key]:raise ValueError('Warm allocation work grows with base')
 return True
if __name__=='__main__':
 d=root/'docs/audits/ai-spatial-delta';h=json.loads((d/'native-history.json').read_text());w=json.loads((d/'native-weld.json').read_text());s=json.loads((d/'scaling.json').read_text());validate(h,w,s)
 mutations=[]
 a=copy.deepcopy(h);a.pop();mutations.append((a,w,s))
 a=copy.deepcopy(w);a[0]['edges'][0][0]=99;mutations.append((h,a,s))
 a=copy.deepcopy(s);a[1]['allocated_bytes']+=1;mutations.append((h,w,a))
 a=copy.deepcopy(s);a[1]['largest_allocation']=1200000;mutations.append((h,w,a))
 a=copy.deepcopy(s);a[1]['publication_rows']+=50000;mutations.append((h,w,a))
 mutations.append((h,w,s[:-1]))
 for a,b,c in mutations:
  try:validate(a,b,c)
  except ValueError:continue
  raise SystemExit('Negative accepted')
 print('PASS: 12 source history states, seven registry weld cases, two allocation inventories, six rejected mutations')
