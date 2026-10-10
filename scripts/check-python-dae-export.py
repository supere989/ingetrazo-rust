#!/usr/bin/env python3
"""COLLADA XML geometry decoded independently and reopened in original runtime."""
import os,sys,json,tempfile,subprocess,importlib.util,xml.etree.ElementTree as ET
from pathlib import Path
from acceptance_context import context
spec=importlib.util.spec_from_file_location("exchange",Path(__file__).with_name("check-exchange-exports.py"));exchange=importlib.util.module_from_spec(spec);spec.loader.exec_module(exchange)
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from formats import igz,dae,obj
NS="{http://www.collada.org/2005/11/COLLADASchema}"
def decode(path):
 root=ET.parse(path).getroot();assert root.find(NS+"asset/"+NS+"up_axis").text=="Z_UP";assert float(root.find(NS+"asset/"+NS+"unit").get("meter"))==1
 triangles=[]
 for mesh in root.iter(NS+"mesh"):
  sources={}
  for source in mesh.findall(NS+"source"):
   array=source.find(NS+"float_array");values=list(map(float,array.text.split()));assert len(values)==int(array.get("count"))
   accessor=source.find(NS+"technique_common/"+NS+"accessor");stride=int(accessor.get("stride"));assert len(values)==stride*int(accessor.get("count"));sources[source.get("id")]=[tuple(values[i:i+stride]) for i in range(0,len(values),stride)]
  bindings={v.get("id"):v.find(NS+"input").get("source")[1:] for v in mesh.findall(NS+"vertices")}
  for primitive in mesh.findall(NS+"triangles"):
   inputs=primitive.findall(NS+"input");stride=max(int(i.get("offset","0")) for i in inputs)+1;vertex=next(i for i in inputs if i.get("semantic")=="VERTEX");points=sources[bindings[vertex.get("source")[1:]]];offset=int(vertex.get("offset","0"));indices=list(map(int,primitive.find(NS+"p").text.split()));assert len(indices)==int(primitive.get("count"))*3*stride
   normal=next(i for i in inputs if i.get("semantic")=="NORMAL");normals=sources[normal.get("source")[1:]];normaloffset=int(normal.get("offset","0"))
   assert all(normals[indices[i+normaloffset]]==(0,0,1) for i in range(0,len(indices),stride))
   for start in range(0,len(indices),stride*3):triangles.append(tuple(points[indices[start+offset+i*stride]] for i in range(3)))
 return triangles
def main():
 app=QApplication.instance() or QApplication([])
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);scene=Scene();mesh=Mesh();mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],[[V(1,1,0),V(3,1,0),V(3,3,0),V(1,3,0)]])
  child=Group(mesh,"Reflected");child.xform=QMatrix4x4();child.xform.scale(-1,1,1);parent=Group(name="Parent");parent.xform=QMatrix4x4();parent.xform.translate(10,20,30);parent.children=[child];hidden=Group(mesh,"Hidden");hidden.hidden=True;hidden.xform=QMatrix4x4();hidden.xform.translate(100,100,100);scene.groups=[parent,hidden]
  source=directory/"source.igz";igz.save_scene(scene,source);out=directory/"export.dae";subprocess.run(["target/debug/ingetrazo","convert",str(source),str(out)],check=True,capture_output=True)
  decoded=exchange.validate(decode(out));assert not decoded["errors"],decoded
  reopened=Scene();dae.load_dae(reopened,out);path=directory/"reopened.obj";obj.save_obj(reopened,path);native=exchange.validate(exchange.decode(path),require_positive_winding=False);assert not native["errors"],native
  wrong=[((6,20,30),(10,20,30),(10,24,30)),((6,20,30),(10,24,30),(6,24,30))];assert exchange.validate(wrong)["errors"]
 report={"provenance":context(),"passed":True,"negative_controls_detected":1,"xml_decoded":decoded,"original_reopened":native,"scope":"COLLADA Z-up metre accessor/index/normal integrity and nested reflected visible holed surface world geometry; original DAE load and OBJ spatial reopen. Original DAE loader flips this open planar surface to -Z during its orient pass; source XML independently retains +Z normals/winding, while original reopening checks spatial geometry. Materials/textures/geolocation and full DAE import remain open."}
 Path("docs/audits/python-dae-export-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
