import json,sys
from PySide6.QtGui import QVector3D
rows=[]
for name,p in [('integer',(1,2,3)),('fractional',(.1,.2,.3)),('large',(1e20,1,1)),('tiny',(1e-30,1e-30,0)),('signed',(-1.25,0,3.75)),('zero',(0,0,0)),('fuzzy-small',(0.000009999,0,0)),('fuzzy-above',(0.000010001,0,0)),('unit-below',(0.999991,0,0)),('unit-above',(1.000011,0,0))]:
 v=QVector3D(*p)
 def xyz(v):return [v.x(),v.y(),v.z()]
 expected={'p':xyz(v),'length':v.length(),'squared':v.lengthSquared(),'add':xyz(v+QVector3D(.2,-.3,.4)),'sub':xyz(v-QVector3D(.2,-.3,.4)),'mul':xyz(v*1.3),'div':xyz(v/1.3)}
 # repr inf is not a Python literal; retain it as float('inf') in assertions.
 code=f"v=QVector3D{p!r}\n"
 for k,expr in [('p','v'),('add','v+QVector3D(.2,-.3,.4)'),('sub','v-QVector3D(.2,-.3,.4)'),('mul','v*1.3'),('div','v/1.3')]:
  code+=f"w={expr}\nassert [w.x(),w.y(),w.z()]=={expected[k]!r}\n"
 for k,expr in [('length','v.length()'),('squared','v.lengthSquared()')]:
  value=repr(expected[k]);value="float('inf')" if value=='inf' else value
  code+=f"assert {expr}=={value}\n"
 for k,w in [('neg',-v),('normalized',v.normalized()),('cross',QVector3D.crossProduct(v,QVector3D(.2,-.3,.4)))]:
  expr={'neg':'-v','normalized':'v.normalized()','cross':'QVector3D.crossProduct(v,QVector3D(.2,-.3,.4))'}[k]
  code+=f"w={expr}\nassert [w.x(),w.y(),w.z()]=={xyz(w)!r}\n"
 code+=f"assert QVector3D.dotProduct(v,QVector3D(.2,-.3,.4))=={QVector3D.dotProduct(v,QVector3D(.2,-.3,.4))!r}\nassert v.isNull()=={v.isNull()!r}\n"
 normalized=QVector3D(v);normalized.normalize()
 code+=f"n=QVector3D(v)\nheld=n\nn.normalize()\nassert held is n\nassert [n.x(),n.y(),n.z()]=={xyz(normalized)!r}\n"
 code+="assert v==QVector3D(v)\nassert v!=QVector3D(7,8,9)\n"
 for op in ['+=','-=','*=','/=']:
  q=QVector3D(v);other=QVector3D(.2,-.3,.4);held=q
  exec('q '+op+' other')
  code+=f"q=QVector3D(v)\nheld=q\nq {op} QVector3D(.2,-.3,.4)\nassert (held is q)=={held is q!r}\nassert [q.x(),q.y(),q.z()]=={xyz(q)!r}\n"
 code+="copy=QVector3D(v)\nassert copy is not v\ncopy.setX(7.25)\ncopy.setY(-2.5)\ncopy.setZ(.125)\nassert [copy.x(),copy.y(),copy.z()]==[7.25,-2.5,.125]\n"
 exec(code,{'QVector3D':QVector3D})
 rows.append({'name':name,'code':code})
print(json.dumps(rows,indent=2))
