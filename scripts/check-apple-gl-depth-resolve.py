#!/usr/bin/env python3
"""Validate the witness inventory; report measured selectors without assuming index0."""
import json, math, sys
r=json.load(open(sys.argv[1]))
assert all(isinstance(r.get(k),str) and r[k] for k in ('renderer','vendor','version'))
assert len(r['cases'])==2
summary=[]
for case,n in zip(r['cases'],(2,4)):
    assert case['requested']==n and case['actual']==n and case['format']==0x88f0
    assert len(case['positions_bottom_origin'])==n
    assert all(len(p)==2 and all(math.isfinite(x) and 0<=x<=1 for x in p) for p in case['positions_bottom_origin'])
    assert len(case['shifts'])==n
    selected=[]
    for row,shift in zip(case['shifts'],range(n)):
        assert row['shift']==shift and len(row['pixels'])==64
        assert all(math.isfinite(x) and 0<=x<=1 for x in row['pixels'])
        matches=[i for i in range(n) if all(abs(x-(1+(i+shift)%n)/(n+1))<=2e-7 for x in row['pixels'])]
        assert len(matches)==1 and matches[0]==row['selected_sample']
        selected.append(matches[0])
    assert len(set(selected))==1, 'Resolve selector varied with sample depth ordering'
    summary.append({'samples':n,'measured_selector':selected[0]})
print(json.dumps({'inventory_passed':True,'observed':summary,'same_metal_device_verified':r['same_metal_device_verified'],'production_policy_authorized':False},indent=2))
