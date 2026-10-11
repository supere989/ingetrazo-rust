import sys,os,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from tests.test_pushpull_slanted import _gable_house,_roof,_wedge,_wedge_faces
from tests.test_pushpull_against_wall import _step_with_square
from tests.test_pushpull_ux import _push
from core import solids
from core.group import Group
from core.orient import is_closed
app=QApplication.instance()or QApplication([])
rows=[]
for name,recipe,d,ctrl in [("ctrl-roof-0.2",lambda:(_gable_house()[0]),-.2,True),("ctrl-step0.5",lambda:_step_with_square(),.5,True),("ctrl-step-0.5",lambda:_step_with_square(),-.5,True),("wedge0--0.4",lambda:_wedge()[0],-.4,False)]:
 value=recipe()
 if isinstance(value,tuple):s,f=value
 else:
  s=value;f=_roof(s) if "roof"in name else _wedge_faces(s)[0]
 vp=_push(s,f,d,ctrl)
 row={"name":name,"status":vp.last_status,"faces":len(s.mesh.faces),"interior":sum(bool(getattr(f,"interior",False))for f in s.mesh.faces),"weak_closed":is_closed(s.mesh),"solid_report":solids.solid_report(s.mesh)}
 try:
  manifold=solids._to_manifold(Group(s.mesh),solids._Sources());row["manifold_volume"]=manifold.volume()
 except Exception as e:row["manifold_error"]=str(e)
 from collections import Counter
 triangles=[tuple(tuple(float(c)for c in (p.x(),p.y(),p.z()))for p in tri)for face in s.mesh.faces if not getattr(face,"interior",False) for tri in face.triangulate(face.normal())]
 directed=Counter((tri[i],tri[(i+1)%3])for tri in triangles for i in range(3))
 undirected=Counter(tuple(sorted((tri[i],tri[(i+1)%3])))for tri in triangles for i in range(3))
 row["triangles"]=triangles
 row["triangle_boundary_errors"]=[{"edge":edge,"incidence":count,"oriented_balance":directed[edge]-directed[(edge[1],edge[0])]}for edge,count in undirected.items() if count!=2 or directed[edge]!=directed[(edge[1],edge[0])]]
 row["polygon_fields"]=[{"vertices":[[p.x(),p.y(),p.z()]for p in face.vertices],"holes":[[[p.x(),p.y(),p.z()]for p in ring]for ring in face.holes],"attrs":face.attrs,"interior":bool(getattr(face,"interior",False))}for face in s.mesh.faces]
 row["edges"]=[{"incident":len(e.faces),"boundary":sum(not getattr(f,"interior",False)for f in e.faces)}for e in s.mesh.edges]
 from core.orient import orient_outward
 changes=orient_outward(s.mesh)
 row["explicit_orient_changes"]=len(changes)
 row["after_explicit_orient_interior"]=sum(bool(getattr(f,"interior",False))for f in s.mesh.faces)
 try:row["after_explicit_orient_volume"]=solids._to_manifold(Group(s.mesh),solids._Sources()).volume()
 except Exception as e:row["after_explicit_orient_error"]=str(e)
 rows.append(row)
print(json.dumps(rows,indent=2));Path(sys.argv[1]).write_text(json.dumps(rows,indent=2)+"\n")
