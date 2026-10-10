#!/usr/bin/env python3
"""Decode Rust export buffers independently using only Python stdlib."""
import base64,json,struct,subprocess,tempfile
from pathlib import Path
from acceptance_context import context

def decode(path):
    raw=path.read_bytes()
    if path.suffix==".glb":
        magic,version,length=struct.unpack_from("<III",raw)
        assert magic==0x46546c67 and version==2 and length==len(raw)
        offset=12;chunks={}
        while offset<len(raw):
            size,kind=struct.unpack_from("<II",raw,offset);offset+=8
            assert size%4==0 and offset+size<=len(raw)
            chunks[kind]=raw[offset:offset+size];offset+=size
        doc=json.loads(chunks[0x4e4f534a]);data=chunks[0x004e4942]
    else:
        assert raw.lstrip().startswith(b"{"),".gltf must be textual JSON"
        doc=json.loads(raw);uri=doc["buffers"][0]["uri"]
        assert uri.startswith("data:application/octet-stream;base64,")
        data=base64.b64decode(uri.split(",",1)[1],validate=True)
    assert len(data)>=doc["buffers"][0]["byteLength"]
    primitive=doc["meshes"][0]["primitives"][0]
    def accessor(index):
        a=doc["accessors"][index];view=doc["bufferViews"][a["bufferView"]]
        count=a["count"]*({"VEC3":3,"SCALAR":1}[a["type"]]);code={5126:"f",5125:"I",5123:"H"}[a["componentType"]]
        offset=view.get("byteOffset",0)+a.get("byteOffset",0)
        return struct.unpack_from("<"+code*count,data,offset)
    coords=accessor(primitive["attributes"]["POSITION"]);encoded=list(zip(coords[::3],coords[1::3],coords[2::3]));points=[(x,-z,y) for x,y,z in encoded];indices=accessor(primitive["indices"])
    assert set(encoded)=={(12.,34.,-23.),(11.,34.,-23.),(12.,34.,-24.)},"glTF must encode original Y-up right-handed metres"
    if "NORMAL" in primitive["attributes"]:
        normals=accessor(primitive["attributes"]["NORMAL"]);assert all(abs(x)<1e-6 and abs(y-1)<1e-6 and abs(z)<1e-6 for x,y,z in zip(normals[::3],normals[1::3],normals[2::3])),"Encoded normals must face +Y"
    expected={(12.,23.,34.),(11.,23.,34.),(12.,24.,34.)}
    assert set(points)==expected,(points,expected)
    assert len(indices)==3 and all(i<len(points) for i in indices)
    a,b,c=[points[i] for i in indices]
    # Parent/child composition plus reflected winding must preserve outward +Z.
    normal=(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
    assert normal>0,"Reflected geometry winding incorrect"
    return {"format":path.suffix,"encoded_y_up_points":encoded,"decoded_world_z_up_points":points,"winding_z":normal}

def main():
    with tempfile.TemporaryDirectory() as temp:
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","export_acceptance_fixture","--",temp],check=True)
        results=[decode(Path(temp)/("fixture"+suffix)) for suffix in (".gltf",".glb")]
    output=Path("docs/audits/export-semantic-evidence.json");output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps({"provenance":context(),"passed":True,"results":results,"scope":"Single reflected nested triangle; no textures/material/hierarchy/full format acceptance."},indent=2)+"\n")
    print("Independent textual glTF and GLB decoded geometry/winding acceptance passed")
if __name__=="__main__":main()
