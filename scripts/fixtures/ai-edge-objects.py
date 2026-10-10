p=QVector3D(0,0,0)
a=Vertex(p)
b=Vertex(QVector3D(3,4,0))
c=Vertex(QVector3D(0,0,0))
assert a.position is not p
p.setX(9)
assert a.position.x()==0
assert a.edges is a.edges and isinstance(a.edges,set)
e=Edge(a,b)
assert e.v0 is a and e.v1 is b
assert e.a is a.position and e.b is b.position
assert e.faces is e.faces and isinstance(e.faces,list)
assert e.length()==5
assert e.other(a) is b and e.other(b) is a and e.other(c) is a
assert e.has(a) and e.has(b) and not e.has(c)
assert e.soft is False and e.hidden is False and e.layer is None and e.curve is None
# Edge construction itself does not insert incidence in the source.
assert not a.edges and not b.edges
a.edges.add(e); b.edges.add(e)
shared=object(); extra=object()
e.faces.extend([shared,shared,extra])
assert a.faces()=={shared,extra} and b.faces()=={shared,extra}
held=e.faces
replacement=[extra]
e.faces=replacement
assert e.faces is replacement and held==[shared,shared,extra]
position=QVector3D(6,8,0)
b.position=position
assert e.b is position and e.length()==10
# Slot payloads preserve arbitrary Python identity, not JSON values.
payload=[1,{'opaque':(2,3)}]
e.layer=payload; e.curve=payload; e.soft=payload; e.hidden=payload
assert e.layer is payload and e.curve is payload and e.soft is payload and e.hidden is payload
replacement_edges=set()
a.edges=replacement_edges
assert a.edges is replacement_edges and a.faces()==set()
e.v0=c
assert e.a is c.position and e.other(a) is c and not e.has(a)
# Direct cycle release is VM GC ownership; the logical owner ledger excludes VM heap.
del a,b,c,e,p,position,held,replacement,replacement_edges,shared,extra,payload
