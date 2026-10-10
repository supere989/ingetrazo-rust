#!/usr/bin/env python3
"""Bounded source/native screen controls; nested source uses flattened world edges.
Occlusion source receives an independent analytic surface predicate.
"""
import sys,json,subprocess,os
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,os.environ.get('INGETRAZO_PYTHON_SOURCE','/work/ingetrazo'))
from core.snap import compute_snap
from PySide6.QtGui import QVector3D as V
from types import SimpleNamespace as N
box=[[[2,-1,0],[6,-1,0]],[[6,-1,0],[6,3,0]]]
cases=[dict(name=n,edges=e,candidate=p,start=[0,0,0],shift=True) for n,e,p in [('midpoint',box,[6,1,0]),('crossing',box,[6,0,0]),('corner-foot',box,[6,-1,0]),('endpoint',[[[4,0,0],[4,2,0]]],[4,0,0]),('empty',[],[9,.6,0]),('backwards',[[[-6,-1,0],[-2,-1,0]],[[-6,-1,0],[-6,3,0]]],[-6,1,0])]]
cases += [dict(name='skew',edges=[[[2,4,0],[5,4,0]],[[3,3,1],[3,6,1]]],candidate=[3.02,4.02,0]),dict(name='cross-real',edges=[[[2,4,0],[5,4,0]],[[3,3,0],[3,6,0]]],candidate=[3.02,4.02,0])]
cases += [dict(name='nested-endpoint',nested=True,edges=[[[0,0,0],[1,1,0]]],candidate=[3.02,4.02,0]),dict(name='occluded-endpoint',occluded=True,edges=[[[3,4,0],[3.5,4.5,0]]],candidate=[3.02,4.02,0])]
p=lambda a:V(*a)
w2p=lambda p:(1000+100*p.x(),1000-100*p.y())
subprocess.run(['cargo','build','--locked','-p','ingetrazo-core','--example','screen_snap_fixture'],check=True)
rows=[]
native=json.loads(subprocess.check_output(['target/debug/examples/screen_snap_fixture'],input=json.dumps(cases).encode()))
for case,rust in zip(cases,native):
 cand=p(case['candidate']);start=p(case['start']) if 'start' in case else None
 r=compute_snap(cand,w2p(cand),N(edges=[N(a=p(a)+V(3,4,0) if case.get('nested') else p(a),b=p(b)+V(3,4,0) if case.get('nested') else p(b),center=False) for a,b in case['edges']]),w2p,9,project_onto_line=lambda s,d:s+d*(V.dotProduct(cand-s,d)/d.lengthSquared()),start_point=start,shift_lock_dir=V(1,0,0) if case.get('shift') else None,edge_threshold_px=14,is_occluded=(lambda p:p.z()<0.5 and 2<p.x()<4 and 3<p.y()<5) if case.get("occluded") else None)
 original=dict(kind=r.kind,point=[r.point.x(),r.point.y(),r.point.z()]);rows.append(dict(input=case,original=original,native=rust))
 assert original['kind']==rust['kind'] and max(abs(a-b) for a,b in zip(original['point'],rust['point']))<1e-6,(case,original,rust)

Path("docs/audits/python-screen-snap-evidence.json").write_text(json.dumps(dict(context=context(),cases=rows,passed=len(rows),scope="10 orthographic source/native controls; nested uses source flattened world edge view, source occlusion analytic predicate; authored by implementation worker, independent validation pending"),indent=2)+"\n")
print(f"Source/native screen controls: {len(rows)}/{len(rows)} PASS")
