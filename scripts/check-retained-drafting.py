#!/usr/bin/env python3
"""Compare fresh native drafting with immutable original rows; no source execution."""
import argparse,ast,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('original');p.add_argument('native');p.add_argument('report');a=p.parse_args()
tree=ast.parse(Path(__file__).with_name('check-python-drafting.py').read_text());ns={'json':json}
helpers=ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ('ring','faces','edge','edges','curves')],type_ignores=[])
exec(compile(helpers,'retained-drafting-boundary','exec'),ns)
original=json.loads(Path(a.original).read_text());native=json.loads(Path(a.native).read_text());rows=[]
assert len(original)==len(native)==22
for x,y in zip(original,native):
    same=all(ns[k](x['original'][field])==ns[k](y[field]) for k,field in [('faces','faces'),('edges','edges'),('curves','edges')])
    rows.append({'case':x['input']['name'],'match':same})
r={'passed':all(x['match'] for x in rows),'cases':rows,'scope':'Retained original oriented faces/holes/attrs, edges and curve groups at existing five-decimal drafting boundary; no source recapture'}
Path(a.report).write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));assert r['passed']
