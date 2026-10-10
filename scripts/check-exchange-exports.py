#!/usr/bin/env python3
"""Independent OBJ/STL/DXF geometry acceptance, including holes/nested reflection."""
import argparse,base64,hashlib,json,math,os,struct,subprocess,sys,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D,QMatrix4x4
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.materials import Material
from core.group import world_mesh
from formats import igz,obj,stl
import ezdxf

def sub(a,b):return tuple(x-y for x,y in zip(a,b))
def cross(a,b):return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def triangle_area(a,b,c):return math.sqrt(sum(x*x for x in cross(sub(b,a),sub(c,a))))/2

def decode(path):
    triangles=[]
    if path.suffix==".obj":
        vertices=[]
        for line in path.read_text().splitlines():
            parts=line.split()
            if not parts:continue
            if parts[0]=="v":vertices.append(tuple(map(float,parts[1:4])))
            elif parts[0]=="f":
                indices=[int(p.split("/")[0]) for p in parts[1:]]
                ring=[vertices[i-1 if i>0 else i] for i in indices]
                triangles += [(ring[0],ring[i],ring[i+1]) for i in range(1,len(ring)-1)]
    elif path.suffix==".stl":
        raw=path.read_bytes();count=struct.unpack_from("<I",raw,80)[0]
        assert len(raw)==84+50*count,"Binary STL triangle count/size mismatch"
        for i in range(count):
            values=struct.unpack_from("<12f",raw,84+50*i)
            normal=values[:3];triangle=(values[3:6],values[6:9],values[9:12]);computed=cross(sub(triangle[1],triangle[0]),sub(triangle[2],triangle[0]))
            assert sum(x*y for x,y in zip(normal,computed))>0,"STL normal does not match winding"
            triangles.append(triangle)
    elif path.suffix==".dxf":
        document=ezdxf.readfile(path)
        assert not document.audit().has_errors,"External DXF parser reports invalid document"
        for face in document.modelspace().query("3DFACE"):
            ring=[tuple(getattr(face.dxf,"vtx"+str(i))) for i in range(4)]
            if ring[3]==ring[2]:ring.pop()
            triangles += [(ring[0],ring[i],ring[i+1]) for i in range(1,len(ring)-1)]
    return triangles

def contains(triangle,x,y):
    a,b,c=triangle
    signs=[(q[0]-p[0])*(y-p[1])-(q[1]-p[1])*(x-p[0]) for p,q in ((a,b),(b,c),(c,a))]
    return min(signs)>=-1e-8 or max(signs)<=1e-8

def validate(triangles,require_positive_winding=True):
    errors=[];area=sum(triangle_area(*t) for t in triangles)
    if abs(area-12)>1e-6:errors.append(f"Expected holed nested surface area12, got {area}")
    if not triangles:errors.append("Required nested transformed surface absent")
    for triangle in triangles:
        if any(abs(p[2]-30)>1e-6 or not (6-1e-6<=p[0]<=10+1e-6 and 20-1e-6<=p[1]<=24+1e-6) for p in triangle):errors.append("Transform/hidden-object bounds incorrect");break
        if require_positive_winding and cross(sub(triangle[1],triangle[0]),sub(triangle[2],triangle[0]))[2]<=0:errors.append("Reflected winding did not preserve +Z orientation");break
    for i in range(16):
        for j in range(16):
            x,y=6+(i+.371)*.25,20+(j+.413)*.25
            expected=not (7<x<9 and 21<y<23)
            if any(contains(t,x,y) for t in triangles)!=expected:errors.append(f"Spatial coverage incorrect at{x},{y}")
    return {"area":area,"triangles":len(triangles),"negative_z_winding":sum(cross(sub(t[1],t[0]),sub(t[2],t[0]))[2]<0 for t in triangles),"errors":errors}

def check_materials(path,expected_asset):
    vertices=[];uvs=[];material_names=[];checks=0
    for line in path.read_text().splitlines():
        fields=line.split()
        if not fields:continue
        if fields[0]=="v":vertices.append(tuple(map(float,fields[1:4])))
        elif fields[0]=="vt":uvs.append(tuple(map(float,fields[1:3])))
        elif fields[0]=="usemtl":material_names.append(fields[1])
        elif fields[0]=="f":
            for field in fields[1:]:
                indices=field.split("/");assert len(indices)>1 and indices[1],"Textured OBJ vertex has no UV"
                point=vertices[int(indices[0])-1];uv=uvs[int(indices[1])-1]
                expected=(0.5*(10-point[0])+0.25,0.5*(point[1]-20)+0.5)
                assert all(abs(a-b)<1e-6 for a,b in zip(uv,expected)),(point,uv,expected);checks+=1
    assert set(material_names)=={"Audit_Paint"},material_names
    definitions=[line.split()[1] for line in path.with_suffix(".mtl").read_text().splitlines() if line.startswith("newmtl ")]
    assert definitions==["Audit_Paint"],definitions
    assets=[]
    for line in path.with_suffix(".mtl").read_text().splitlines():
        fields=line.split()
        if not fields:continue
        if fields[0]=="Kd":assert all(abs(float(x)-expected)<1e-6 for x,expected in zip(fields[1:],(.2,.4,.8)))
        elif fields[0]=="d":assert abs(float(fields[1])-.6)<1e-6
        elif fields[0]=="map_Kd":
            data=(path.parent/fields[1]).read_bytes();assert data==expected_asset;assets.append(hashlib.sha256(data).hexdigest())
    assert assets
    reopened=Scene();obj.load_obj(reopened,path)
    faces=[face for group in reopened.groups for face in world_mesh(group).faces]+list(reopened.mesh.faces)
    assert faces
    for face in faces:
        tex=face.attrs["texture"];uvw=tex["uvw"]
        for point in face.vertices:
            actual=(uvw[0]*point.x()+uvw[1]*point.y()+uvw[2]*point.z()+uvw[3],uvw[4]*point.x()+uvw[5]*point.y()+uvw[6]*point.z()+uvw[7])
            expected=(0.5*(10-point.x())+0.25,0.5*(point.y()-20)+0.5)
            assert all(abs(a-b)<1e-5 for a,b in zip(actual,expected))
        assert Path(face.attrs["texture"]["path"]).read_bytes()==expected_asset
    return {"affine_vertex_uv_checks":checks,"asset_hashes":sorted(set(assets)),"original_python_reopened_faces":len(faces),"color":[.2,.4,.8],"opacity":.6}

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--binary",default="target/debug/ingetrazo");parser.add_argument("--output",default="docs/audits/exchange-export-evidence.json");args=parser.parse_args()
    app=QApplication.instance() or QApplication([]);results=[]
    with tempfile.TemporaryDirectory() as temp:
        directory=Path(temp);scene=Scene();mesh=Mesh();outer=[QVector3D(*p) for p in [(0,0,0),(4,0,0),(4,4,0),(0,4,0)]];inner=[QVector3D(*p) for p in [(1,1,0),(3,1,0),(3,3,0),(1,3,0)]];mesh.add_face(outer,[inner])
        image=directory/"source.png";image.write_bytes(base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j15kAAAAASUVORK5CYII="))
        mesh.faces[0].attrs={"mat":"Audit Paint","texture":{"path":str(image),"uvw":[0.5,0,0,0.25,0,0.5,0,0.5]}}
        scene.materials["Audit Paint"]=Material("Audit Paint",(0.2,0.4,0.8),opacity=0.6)
        child=Group(mesh,"reflected child");child.xform=QMatrix4x4();child.xform.scale(-1,1,1)
        parent=Group(name="translated parent");parent.xform=QMatrix4x4();parent.xform.translate(10,20,30);parent.children=[child]
        hidden=Group(mesh,"hidden object");hidden.hidden=True;hidden.xform=QMatrix4x4();hidden.xform.translate(100,100,100)
        scene.groups=[parent,hidden];source=directory/"source.igz";igz.save_scene(scene,source)
        original_results={}
        for extension,save in [("obj",obj.save_obj),("stl",stl.save_stl)]:
            path=directory/("original."+extension);save(scene,path);original_results[extension]=validate(decode(path),require_positive_winding=False);assert not original_results[extension]["errors"],original_results[extension]
        for extension in ("obj","stl","dxf"):
            output=directory/("rust."+extension);proc=subprocess.run([args.binary,"convert",str(source),str(output)],capture_output=True,text=True)
            result=validate(decode(output)) if proc.returncode==0 else {"errors":[proc.stderr]}
            if extension=="obj" and proc.returncode==0:
                result["material_checks"]=check_materials(output,image.read_bytes())
            result["format"]=extension;result["exit_code"]=proc.returncode;results.append(result)
        # Original OBJ loader drops d and textured Kd: verify raw MTL opacity
        # above, and independently reopen color-only material through that loader.
        mesh.faces[0].attrs.pop("texture");color_source=directory/"colors.igz";igz.save_scene(scene,color_source)
        color_output=directory/"colors.obj";subprocess.run([args.binary,"convert",str(color_source),str(color_output)],check=True,capture_output=True)
        colored=Scene();obj.load_obj(colored,color_output)
        colored_faces=[f for group in colored.groups for f in world_mesh(group).faces]+list(colored.mesh.faces)
        assert colored_faces and all(all(abs(float(a)-b)<1e-6 for a,b in zip(f.attrs["color"],(.2,.4,.8))) for f in colored_faces)
        distinct=Scene();distinct.materials["First Paint"]=Material("First Paint",(.2,.4,.8));distinct.materials["Second Paint"]=Material("Second Paint",(.2,.4,.8))
        for i,name in enumerate(("First Paint","Second Paint")):
            distinct.mesh.add_face([QVector3D(i*2,0,0),QVector3D(i*2+1,0,0),QVector3D(i*2,1,0)])
            distinct.mesh.faces[-1].attrs={"mat":name}
        distinct_source=directory/"distinct.igz";igz.save_scene(distinct,distinct_source)
        distinct_output=directory/"distinct.obj";subprocess.run([args.binary,"convert",str(distinct_source),str(distinct_output)],check=True,capture_output=True)
        definitions=[line.split()[1] for line in distinct_output.with_suffix(".mtl").read_text().splitlines() if line.startswith("newmtl ")]
        assert set(definitions)=={"First_Paint","Second_Paint"},definitions
        bad=directory/"missing.igz";bad.write_text(json.dumps({"igz_format":1,"app_version":"independent","scene":{"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"texture":{"embed":"absent.png"}}]}}))
        preserved=directory/"preserved.obj";preserved.write_bytes(b"original-object");preserved.with_suffix(".mtl").write_bytes(b"original-material")
        refused=subprocess.run([args.binary,"convert",str(bad),str(preserved)],capture_output=True,text=True)
        assert refused.returncode!=0 and preserved.read_bytes()==b"original-object" and preserved.with_suffix(".mtl").read_bytes()==b"original-material","Missing texture must refuse before OBJ/MTL truncation"
        # An unholed rectangle must fail area and spatial coverage controls.
        wrong=[((6,20,30),(10,20,30),(10,24,30)),((6,20,30),(10,24,30),(6,24,30))]
        assert validate(wrong)["errors"]
    report={"provenance":context(),"passed":all(not r["errors"] for r in results),"negative_controls_detected":2,"original_export_reference":original_results,"results":results,"scope":"World-space holed reflected nested surface, hidden top-level object; OBJ/STL original export comparison and external DXF parsing. Rust +Z reflected winding is a correction to original -Z open-face winding; OBJ named color/opacity raw MTL, exact texture asset hashes/local affine UVs and original-runtime color-only/texture-UV reopen checked (original loader ignores d and textured Kd); loose wires, broad importer entities and sheet DXF remain open."}
    Path(args.output).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({"passed":report["passed"],"results":[{"format":r["format"],"errors":len(r["errors"])} for r in results]},indent=2))
    return 0 if report["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
