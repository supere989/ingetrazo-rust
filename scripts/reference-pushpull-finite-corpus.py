"""Capture missing finite slanted and ordered wall regression workflows once."""
import os,sys,json,itertools
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from tests import test_pushpull_slanted as slanted,test_pushpull_orient as orient
app=QApplication.instance() or QApplication([])
def fields(m):return [{"vertices":[[v.x(),v.y(),v.z()]for v in f.vertices],"holes":[[[v.x(),v.y(),v.z()]for v in h]for h in f.holes],"attrs":f.attrs,"interior":bool(f.interior)}for f in m.faces]
rows=[];results=[];workflow="";step=0
for module in [slanted,orient]:
 old=module._push
 def wrapped(scene,hist,face,distance,old=old):
  global step
  # Builders' initial extrusion is already covered by retained captures.
  before=fields(scene.mesh);index=scene.mesh.faces.index(face);count=len(hist.undo_stack)
  result=old(scene,hist,face,distance)
  if len(before)>1:
   rows.append({"name":workflow+"/"+str(step),"workflow":workflow,"step":step,"source":before,"face":index,"distance":distance,"ctrl":False,"result":fields(scene.mesh),"source_undo_delta":len(hist.undo_stack)-count})
   step+=1
  return result
 module._push=wrapped
calls=[("roof-thicken-flush",slanted.test_roof_pushed_back_flush_restores_house,())]
for perm in itertools.permutations(range(3)):calls.append(("wedge-"+"".join(map(str,perm)),slanted.test_wedge_push_orders,(perm,)))
for perm in orient._seqs():calls.append(("prism-"+"".join(map(str,perm)),orient.test_side_wall_push_orders_commit_oriented_solid,(perm,)))
for perm in [(0,0,1),(1,1,2),(2,2,0),(0,1,0)]:calls.append(("repeat-"+"".join(map(str,perm)),orient.test_repeated_wall_push_commits_oriented_solid,(perm,)))
for name,fn,args in calls:
 workflow=name;step=0
 try:fn(*args);error=None
 except Exception as e:error=type(e).__name__+": "+str(e)
 results.append({"workflow":name,"source_test_error":error})
Path(sys.argv[1]).write_text(json.dumps({"cases":rows,"workflows":results},indent=2)+"\n")
print(len(results),"workflows",len(rows),"operations",[r for r in results if r["source_test_error"]])
