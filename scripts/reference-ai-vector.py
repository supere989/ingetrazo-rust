import json,sys
from PySide6.QtGui import QVector3D
rows=[]
for name,p in [('integer',(1,2,3)),('fractional',(.1,.2,.3)),('large',(1e20,1,1)),('tiny',(1e-30,1e-30,0)),('signed',(-1.25,0,3.75)),('zero',(0,0,0))]:
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
 code+="copy=QVector3D(v)\nassert copy is not v\ncopy.setX(7.25)\ncopy.setY(-2.5)\ncopy.setZ(.125)\nassert [copy.x(),copy.y(),copy.z()]==[7.25,-2.5,.125]\n"
 exec(code,{'QVector3D':QVector3D})
 rows.append({'name':name,'code':code})
print(json.dumps(rows,indent=2))
