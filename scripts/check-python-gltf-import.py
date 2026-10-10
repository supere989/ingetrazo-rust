#!/usr/bin/env python3
"""Independent authored glTF containers compared to original importer world geometry/UVs."""
import base64,json,struct,subprocess,tempfile,os,sys,copy,hashlib
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.group import world_mesh
from formats import glb,igz
PNG=base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j15kAAAAASUVORK5CYII=")
def triangles(scene):
 return [face for group in scene.groups for face in world_mesh(group).faces]+list(scene.mesh.faces)
def recipe(face):
 uvw=face.attrs["texture"]["uvw"];return sorted((tuple(round(c,6) for c in (p.x(),p.y(),p.z())),tuple(round(uvw[i]*p.x()+uvw[i+1]*p.y()+uvw[i+2]*p.z()+uvw[i+3],6) for i in (0,4))) for p in face.vertices)
def container(path,doc,data):
 if path.suffix==".glb":
  js=json.dumps(doc,separators=(",",":")).encode();js+=b" "*((-len(js))%4);binary=data+b"\0"*((-len(data))%4)
  path.write_bytes(struct.pack("<III",0x46546c67,2,12+8+len(js)+8+len(binary))+struct.pack("<II",len(js),0x4e4f534a)+js+struct.pack("<II",len(binary),0x004e4942)+binary)
 else:path.write_text(json.dumps(doc))
def main():
 app=QApplication.instance() or QApplication([]);rows=[]
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);os.environ["XDG_CACHE_HOME"]=str(directory/"cache")
  data=b"".join(struct.pack("<3f2H4x",*p,*uv) for p,uv in [((0,0,0),(0,0)),((1,0,0),(65535,0)),((0,0,1),(0,65535))])+struct.pack("<3H",0,1,2)
  template={"asset":{"version":"2.0","extras":{"title":"Independent reference"}},"buffers":[{"byteLength":len(data)}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":60,"byteStride":20},{"buffer":0,"byteOffset":60,"byteLength":6}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,0,1]},{"bufferView":0,"byteOffset":12,"componentType":5123,"normalized":True,"count":3,"type":"VEC2"},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],"images":[{"uri":"pixel.png"}],"textures":[{"source":0}],"materials":[{"alphaMode":"BLEND","pbrMetallicRoughness":{"baseColorFactor":[.2,.4,.8,.6],"baseColorTexture":{"index":0,"extensions":{"KHR_texture_transform":{"offset":[.25,.5],"scale":[2,3]}}}}}],"extensionsUsed":["KHR_texture_transform"],"meshes":[{"primitives":[{"attributes":{"POSITION":0,"TEXCOORD_0":1},"indices":2,"material":0}]}],"nodes":[{"translation":[2,3,4],"children":[1]},{"mesh":0,"scale":[-2,3,1]},{"translation":[10,3,4],"mesh":0}],"scenes":[{"nodes":[0,2]}],"scene":0}
  (directory/"pixel.png").write_bytes(PNG);(directory/"mesh.bin").write_bytes(data)
  for name in ("external.gltf","embedded.gltf","embedded.glb"):
   doc=copy.deepcopy(template);source=directory/name
   if name=="external.gltf":doc["buffers"][0]["uri"]="mesh.bin"
   elif name=="embedded.gltf":doc["buffers"][0]["uri"]="data:application/octet-stream;base64,"+base64.b64encode(data).decode()
   if name!="external.gltf":doc["images"][0]["uri"]="data:image/png;base64,"+base64.b64encode(PNG).decode()
   container(source,doc,data);original=Scene();glb.load_glb(original,source);out=directory/(name+".igz");subprocess.run(["target/debug/ingetrazo","convert",str(source),str(out)],check=True,capture_output=True)
   rust=Scene();igz.load_into(rust,out);a,b=triangles(original),triangles(rust);assert len(a)==len(b)==2
   assert sorted(recipe(f) for f in a)==sorted(recipe(f) for f in b),(name,[recipe(f) for f in a],[recipe(f) for f in b])
   assert rust.groups[0].name=="Independent reference"
   for face in b:
    a,bp,c=face.vertices;assert ((bp-a).x()*(c-a).y()-(bp-a).y()*(c-a).x())<0,"Reflection must retain the authored outward -Z normal"
    assert abs(face.attrs["opacity"]-.6)<1e-6
    assert all(abs(x-y)<1e-6 for x,y in zip(face.attrs["color"],(.2,.4,.8)))
    assert Path(face.attrs["texture"]["path"]).read_bytes()==PNG
   rows.append({"case":name,"original_reopened_triangles":2,"world_vertices_and_uvs_equal":True,"asset_sha256":hashlib.sha256(PNG).hexdigest()})
  for name in ("range","required-extension"):
   doc=copy.deepcopy(template);doc["buffers"][0]["uri"]="mesh.bin"
   if name=="range":doc["bufferViews"][0]["byteLength"]=1
   else:doc["extensionsRequired"]=["KHR_draco_mesh_compression"]
   source=directory/(name+".gltf");container(source,doc,data);out=directory/(name+".igz");out.write_bytes(b"original-destination")
   result=subprocess.run(["target/debug/ingetrazo","convert",str(source),str(out)],capture_output=True,text=True);assert result.returncode and out.read_bytes()==b"original-destination"
   rows.append({"case":name,"refused":True,"destination_preserved":True})
 report={"provenance":context(),"passed":True,"negative_controls_detected":2,"cases":rows,"scope":"Authored GLB/textual glTF external and embedded buffers/images; two shared mesh instances, nested reflected TRS, strided normalized ushort UVs, KHR texture offset/scale, PBR color/opacity and raw image bytes. Original textured importer drops PBR color, so color is independently checked against authored input. Rigging/animations/DAE and broader primitives remain open."}
 Path("docs/audits/python-gltf-import-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
