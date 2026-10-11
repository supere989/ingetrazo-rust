m=mesh
f=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])
e=m.edges[0]
eq_calls=[]
mode='false'
class EqualPayload:
 def __eq__(self,other):
  eq_calls.append('eq')
  if mode=='raise': raise ValueError('eq-failure')
  if mode=='mutating-same': m.edges[:]=list(m.edges)
  if mode=='mutating-field': e.hidden=True
  return mode=='true'
oldp=EqualPayload()
e.soft=oldp
