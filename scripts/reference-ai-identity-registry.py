"""Actual source identity-registry authority; native owner-only prerequisite."""
import json,sys
sys.path.insert(0,'/work/ingetrazo')
from core.mesh import Mesh,_key
from PySide6.QtGui import QVector3D
m=Mesh();m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])
a,b=m.vertices[:2];registry=m._registry
observations={'genuine_vertex_identity':registry[_key(QVector3D())] is a}
a.position.setX(5)
observations['raw_move_preserves_old_key']=registry[_key(QVector3D())]is a and _key(a.position)not in registry
# Held registry references outlive public/controller ownership.
del m
observations['detached_controller_lifetime']=registry[_key(QVector3D())]is a and registry[_key(QVector3D(1,0,0))]is b
registry[_key(QVector3D())]=a;registry[_key(QVector3D())]=b
observations['duplicate_key_last_wins']=registry[_key(QVector3D())]is b
print(json.dumps({'observations':observations,'production_enabled':False}))
