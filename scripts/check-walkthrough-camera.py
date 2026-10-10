#!/usr/bin/env python3
import json,sys,math
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text())["cases"]
n=json.loads(Path(sys.argv[2]).read_text())["cases"]
assert len(s)==len(n)==10
rows=[]
for source,native in zip(s,n,strict=True):
 a=source["after"]
 diffs=[abs(x-y)for k in ("target","eye","forward")for x,y in zip(a[k],native[k],strict=True)]+[abs(a[k]-native[k])for k in ("yaw","pitch","distance")]
 assert all(math.isfinite(x)for x in diffs)
 rows.append(dict(operation=source["op"],max_absolute_difference=max(diffs),passed=max(diffs)<=2e-5))
report=dict(cases=rows,passed=all(r["passed"]for r in rows),scope="Explicit absolute2e-5 numeric bound for Qt float32 versus native float64; not exact/rendered/fullP17 parity")
print(json.dumps(report))
sys.exit(0 if report["passed"]else 1)
