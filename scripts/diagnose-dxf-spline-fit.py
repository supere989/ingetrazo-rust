#!/usr/bin/env python3
"""Replay measured source equation systems; no original handlers or new inputs."""
import json,math,sys
from pathlib import Path
records=json.loads(Path(sys.argv[1]).read_text());results=[]
for r in records:
 for reciprocal in [False,True]:
  for fused in [False,True]:
   a=[row[:]for row in r['a']];b=[row[:]for row in r['b']];n=len(a);piv=[]
   sub=lambda v,c,x:math.fma(-c,x,v)if fused else v-c*x
   for k in range(n):
    pivot=max(range(k,n),key=lambda i:abs(a[i][k]));a[k],a[pivot]=a[pivot],a[k];piv.append(pivot)
    for i in range(k+1,n):
     f=a[i][k]*(1./a[k][k])if reciprocal else a[i][k]/a[k][k];a[i][k]=f
     for j in range(k+1,n):a[i][j]=sub(a[i][j],f,a[k][j])
   for k,p in enumerate(piv):b[k],b[p]=b[p],b[k]
   for i in range(n):
    for j in range(i):
     for c in range(3):b[i][c]=sub(b[i][c],a[i][j],b[j][c])
   for i in reversed(range(n)):
    for j in reversed(range(i+1,n)):
     for c in range(3):b[i][c]=sub(b[i][c],a[i][j],b[j][c])
    for c in range(3):b[i][c]=b[i][c]*(1./a[i][i])if reciprocal else b[i][c]/a[i][i]
   diffs=[{'point':i,'axis':c,'actual':b[i][c],'source':r['expected'][i][c]}for i in range(n)for c in range(3)if b[i][c]!=r['expected'][i][c]]
   results.append({'case':r['case'],'reciprocal':reciprocal,'fused':fused,'control_differences':diffs})
assert next(r for r in results if r['case']=='fit-natural'and r['reciprocal']and r['fused'])['control_differences']==[]
print(json.dumps({'source_equations':2,'results':results},indent=2))
