#!/usr/bin/env python3
"""Frozen actual Qt perspective projection and native screen inference protocol.
Implementation-worker evidence; independent checker review remains required.
"""
import sys,os,json,math,subprocess
from pathlib import Path
from types import SimpleNamespace as N
sys.path.insert(0,os.environ.get('INGETRAZO_PYTHON_SOURCE','/work/ingetrazo'))
from PySide6.QtGui import QVector3D as V
from core.camera import OrbitCamera
from core.mesh import Mesh
from core.snap import compute_snap
from views.viewport import Viewport
from acceptance_context import context
class Stub:
 width=lambda self:1000
 height=lambda self:700
 _world_to_pixel=Viewport._world_to_pixel
 _pixel_to_ray=Viewport._pixel_to_ray
 _project_to_lock_line=Viewport._project_to_lock_line
 _center_of_edge=Viewport._center_of_edge
 _valid_center_ref=Viewport._valid_center_ref
 _center_hint_px=Viewport._center_hint_px
 _is_occluded=Viewport._is_occluded
 _effective_style=lambda self:N(face_mode="shaded")
 _pick_index=lambda self:N(tri_v0=None,own_spans=None)
 CENTER_MIN_RADIUS_PX=Viewport.CENTER_MIN_RADIUS_PX
 CENTER_HINT_PX=Viewport.CENTER_HINT_PX
 def __init__(self):
  self.camera=OrbitCamera();self.camera.target=V(2,3,0);self.camera.distance=10;self.camera.yaw=-math.pi/2;self.camera.pitch=math.pi/6;self.camera.aspect=1000/700;self._center_ref=None
 def camera_json(self):
  c=self.camera
  return dict(target=arr(c.target),up=arr(c.up),**{k:getattr(c,k) for k in ['distance','yaw','pitch','fov_deg','aspect','znear','zfar','perspective','two_point']})
def arr(p):return [p.x(),p.y(),p.z()]
def q(p):return V(*p)
def step(v,world,offset=(0,0),**extras):
 px=v._world_to_pixel(world);assert px is not None
 pixel=[px[0]+offset[0],px[1]+offset[1]];o,d=v._pixel_to_ray(*pixel)
 candidate=o+d*(-o.z()/d.z()) if abs(d.z())>1e-6 else world
 return dict(pixel=pixel,candidate=arr(candidate),time=0,**extras)
v=Stub();cases=[]
def add(name,edges,steps,**extra):cases.append(dict(name=name,edges=edges,steps=steps,camera=v.camera_json(),**extra))
a,b=V(1,2,0),V(4,3,3)
add('perspective-ray-edge',[[arr(a),arr(b)]],[step(v,a+(b-a)*.35)])
add('perspective-endpoint-within-9px',[[arr(a),arr(b)]],[step(v,a,(8,0))])
add('perspective-midpoint',[[arr(a),arr(b)]],[step(v,(a+b)*.5)])
add('perspective-crossing',[[[1,3,0],[4,3,0]],[[2,2,0],[2,5,0]]],[step(v,V(2,3,0),(.3,.3))])
add('perspective-skew-crossing',[[[1,3,0],[4,3,0]],[[2,2,1],[2,5,1]]],[step(v,V(2,3,0),(.3,.3))])
for depth in [.01,.05,.2]:
 point=v.camera.eye()+v.camera.forward()*depth
 add(f'eye-depth-{depth}',[[arr(point),arr(point+V(.1,0,0))]],[step(v,point)],clip_probe=depth<.1)
center=V(2,3,1)
for radius in [1.2,.01]:
 pts=[center+V(radius*math.cos(i*math.tau/24),radius*math.sin(i*math.tau/24),0) for i in range(24)]
 edges=[[arr(p),arr(pts[(i+1)%24])] for i,p in enumerate(pts)]
 steps=[step(v,pts[0],acquire=True),step(v,pts[0],acquire=True),step(v,center)]
 steps[1]['time']=.3;steps[2]['time']=.4
 if radius>1:
  steps.extend([step(v,center+V(1,0,0),move=[1,0,0]),step(v,center+V(1,0,0),delete=True)])
 add(f'curve-center-radius-{radius}',edges,steps,curve=True)
# Held-out malformed curve: one shared-ID spur destroys the radial fit.
pts=[center+V(1.2*math.cos(i*math.tau/24),1.2*math.sin(i*math.tau/24),0) for i in range(24)]
edges=[[arr(p),arr(pts[(i+1)%24])] for i,p in enumerate(pts)]
edges.append([arr(pts[0]),arr(center+V(3,0,0))])
add('malformed-circle-spur',edges,[step(v,pts[0],acquire=True),step(v,center)],curve=True)
Path('docs/audits/perspective-screen-inputs.json').write_text(json.dumps(cases,indent=2)+'\n')
subprocess.run(['cargo','build','--locked','-p','ingetrazo-core','--example','perspective_screen_fixture'],check=True)
native=json.loads(subprocess.check_output(['target/debug/examples/perspective_screen_fixture'],input=json.dumps(cases).encode()))
rows=[];failures=[]
for case,n in zip(cases,native):
 stub=Stub();mesh=Mesh();stub.scene=N(mesh=mesh,version=0)
 for a,b in case['edges']:
  edge=mesh.add_edge(q(a),q(b))
  if case.get('curve'):edge.curve=71
 original=[];memory=[];dwell=None
 for s in case['steps']:
  if 'move' in s:
   for vertex in mesh.vertices:vertex.position+=q(s['move'])
   stub.scene.version+=1
  if s.get('delete'):mesh.clear();stub.scene.version+=1
  hover=None
  if s.get('acquire') and mesh.edges:
   hover=stub._center_of_edge(mesh.edges[0],mesh);stub._center_ref=hover
  ref=stub._valid_center_ref();edges=[N(a=e.a,b=e.b,center=False) for e in mesh.edges]
  if ref is not None:
   px=stub._world_to_pixel(ref[0])
   if px is not None and stub._center_hint_px(ref,px)>0:edges.append(N(a=ref[0],b=ref[0],center=True))
  pixel=s['pixel']
  snap=compute_snap(q(s['candidate']),pixel,N(edges=edges),stub._world_to_pixel,9,project_onto_line=lambda a,d:stub._project_to_lock_line(a,d,*pixel),edge_threshold_px=14,is_occluded=stub._is_occluded)
  point=snap.point if snap.kind in Viewport._ENCOURAGING_KINDS else None
  if hover is not None:point=hover[0]
  if point is None:dwell=None
  elif dwell is None or (dwell[0]-point).length()>1e-6:dwell=(V(point),s['time'])
  elif s['time']-dwell[1]>=.25:
   memory=[p for p in memory if (p-point).length()>1e-6];memory.append(V(point));memory=memory[-2:]
  original.append(dict(kind=snap.kind,point=arr(snap.point),encouraged=[arr(p) for p in memory]))
 matches=[]
 for a,b in zip(original,n['steps']):
  match=a['kind']==b['kind'] and max(abs(x-y) for x,y in zip(a['point'],b['point']))<1e-4 and len(a['encouraged'])==len(b['encouraged']) and all(max(abs(x-y)for x,y in zip(p,q))<1e-4 for p,q in zip(a['encouraged'],b['encouraged']))
  matches.append(match)
 rows.append(dict(name=case['name'],original=original,native=n['steps'],matches=matches,clip_probe=case.get('clip_probe',False)))
 if not all(matches):failures.append(case['name'])
 print(case['name'],matches)
# Independent controls reject implementations using screen-linear interpolation
# or indiscriminately offering the fitted centre of malformed/tiny curves.
edge_case=rows[0]['native'][0];a,b=map(q,cases[0]['edges'][0]);pa,pb=map(v._world_to_pixel,(a,b));pixel=cases[0]['steps'][0]['pixel'];dx,dy=pb[0]-pa[0],pb[1]-pa[1];t=((pixel[0]-pa[0])*dx+(pixel[1]-pa[1])*dy)/(dx*dx+dy*dy);wrong=a+(b-a)*t
assert (wrong-q(edge_case['point'])).length()>.02,'Screen-linear interpolation negative control did not separate perspective depth'
assert rows[-1]['original'][-1]['kind']!='center' and rows[-1]['native'][-1]['kind']!='center'
assert rows[-2]['original'][2]['kind']!='center' and rows[-2]['native'][2]['kind']!='center'
report=dict(context=context(),cases=rows,failed=failures,passed=not failures,negative_controls=dict(screen_linear_distance=(wrong-q(edge_case['point'])).length(),malformed_circle_rejected=True,tiny_center_quiet=True),scope='Actual original Qt perspective camera projection/ray/lock-line functions; center actual mesh curve fit/cache/hint, caller acquisition explicit recipe, timer modeled250ms; own-worker protocol, independent UI picking and wallclock validation pending')
Path('docs/audits/python-perspective-screen-evidence.json').write_text(json.dumps(report,indent=2)+'\n')
print('Retained failures:',failures)

if failures and '--retain-open' not in sys.argv:raise SystemExit(1)
assert all(name.startswith('eye-depth-') for name in failures),failures
