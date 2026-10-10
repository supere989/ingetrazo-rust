events=[]
saved=[]
class Payload:
 def __init__(self,e): self.e=e
 def __del__(self):
  e=self.e
  events.append((e.v0 is self.a, e.v1 is self.b, e in e.v0.edges, e.layer is self, e.curve is self, e.faces is self, e.soft is self, e.hidden is self))
  saved.append(e)
a=Vertex(QVector3D())
b=Vertex(QVector3D(1,0,0))
e=Edge(a,b)
a.edges.add(e)
b.edges.add(e)
p=Payload(e)
e.layer=p
e.curve=p
e.faces=p
e.soft=p
e.hidden=p
# Endpoints are already identity observable; the helper uses held Python aliases.
p.a=a
p.b=b
del a,b,e,p
proof_collect_gc()
assert events==[(True,True,True,True,True,True,True,True)], events
assert len(saved)==1, saved
assert saved[0].layer.e is saved[0]
assert saved[0].v0 is saved[0].layer.a
assert saved[0].v1 is saved[0].layer.b
saved.clear()
proof_collect_gc()
assert len(events)==1
