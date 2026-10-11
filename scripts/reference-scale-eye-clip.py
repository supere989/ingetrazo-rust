# SPDX-License-Identifier: GPL-3.0-or-later
import sys,json,types
from pathlib import Path
from PySide6.QtGui import QMatrix4x4 as M,QVector3D as V
from views.viewport import Viewport
m=M(1,0,0,0,0,1,0,0,0,0,1,0,0,0,1,0)
camera=types.SimpleNamespace(projection_matrix=lambda:m,view_matrix=lambda:M())
vp=types.SimpleNamespace(camera=camera);rows=[]
for name,a,b in [('crossing',[-1,0,-1],[1,0,1]),('reversed',[1,0,1],[-1,0,-1]),('behind',[-1,0,-2],[1,0,-1]),('front',[-1,0,1],[1,0,2])]:
 result=Viewport._clip_segment_front(vp,V(*a),V(*b))
 rows.append(dict(name=name,a=a,b=b,result=None if result is None else [[p.x(),p.y(),p.z()]for p in result]))
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','matrix':list(m.data()),'cases':rows},indent=2)+'\n')
