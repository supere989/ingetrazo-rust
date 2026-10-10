"""Actual original GC identity-graph lifetime controls; no Rust ledger proxy."""
import gc,json,sys
sys.path.insert(0,'/work/ingetrazo')
from core.mesh import Vertex,Edge,Mesh
from PySide6.QtGui import QVector3D
gc.disable();gc.collect()
def live():
 objects=gc.get_objects()
 return [sum(type(o)is Vertex for o in objects),sum(type(o)is Edge for o in objects)]
def scope():return dict(Vertex=Vertex,Edge=Edge,Mesh=Mesh,QVector3D=QVector3D)
c=scope();before=live();assert before==[0,0],before
exec("a=Vertex(QVector3D())\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)\nheld=a\ndel a,b,e",c)
assert live()==[2,1];gc.collect();assert live()==[2,1]
exec("assert next(iter(held.edges)).v0 is held\ndel held",c)
assert live()==[2,1];gc.collect();assert live()==[0,0]
slot_cases=[]
for field in ['faces','layer','curve','soft','hidden']:
 exec(f"a=Vertex(QVector3D())\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\ne.{field}={{e}}\ndel a,b,e",c)
 assert live()==[2,1];gc.collect();assert live()==[0,0];slot_cases.append(field)
exec("private=Mesh()\nf=private.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\na=f.loop[0]\nb=f.loop[1]\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)\ne.faces.append(f)\ndel private,f,a,b,e",c)
# Original also generated three actual boundary Edge objects maintained by Mesh.
assert live()==[3,4];gc.collect();assert live()==[0,0]
other=scope();recipe="a=Vertex(QVector3D())\nb=Vertex(QVector3D(1,0,0))\ne=Edge(a,b)\na.edges.add(e)\nb.edges.add(e)"
exec(recipe,c);exec(recipe,other);assert live()==[4,2]
c.clear();gc.collect();assert live()==[2,1]
exec("assert e.v0 is a and e in a.edges and e.v1 is b and e in b.edges",other)
other.clear();gc.collect();assert live()==[0,0]
print(json.dumps({'generated_face_vertex_edge_cycle_collected':True,'opaque_slot_cycles_collected':slot_cases,'external_alias_retains_cycle':True,'actual_cycle_survives_without_roots_before_gc':True,'actual_payloads_destroyed_after_gc':True,'controller_drop_preserves_other_scope':True,'actual_final_live_payloads':[0,0],'production_enabled':False,'total_heap_budget_claimed':False},indent=2))
