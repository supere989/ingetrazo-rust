#!/usr/bin/env python3
"""Fresh and edited Rust IGZ docs actually opened by original Python runtime."""
import json,os,subprocess,sys,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz

def main():
    app=QApplication.instance() or QApplication([])
    with tempfile.TemporaryDirectory() as temp:
        directory=Path(temp)
        document={"igz_format":1,"app_version":"independent-original","scene":{"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"opaque_face":{"id":23}}],"protos":[{"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]]}]}],"groups":[{"name":"original instance","uid":"persisted-audit-id","proto":0,"xform":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]}],"opaque_document":{"value":29}}}
        document["scene"]["geo_points"]=[{"position":[1,2,3],"name":"original point","opaque_point":43}]
        document["scene"]["geo_paths"]=[{"points":[[0,0,0],[1,0,0],[1,1,0]],"name":"original path","closed":True,"surface":"draped","opaque_path":47}]
        document["scene"]["compositions"]=[{"name":"original sheet","paper":"A4","landscape":True,"frames":[{"uid":"persisted-frame-id","x_mm":20,"y_mm":21,"w_mm":100,"h_mm":80,"scale_n":100,"opaque_frame":53}],"opaque_sheet":59}]
        (directory/"original.igz").write_text(json.dumps(document))
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","edit_acceptance_fixture","--",temp],check=True)
        edited=Scene();igz.load_into(edited,directory/"edited.igz")
        assert any(abs(v.position.z()-2)<1e-6 for v in edited.mesh.vertices),"Edited loose geometry stale in Python"
        group=edited.groups[0]
        assert group.name=="edited instance" and group.uid=="persisted-audit-id"
        assert (group.xform[0,3],group.xform[1,3],group.xform[2,3])==(7,8,9),"Edited transform stale/ignored by original"
        assert edited.geo_points[0].position.z()==17 and edited.geo_points[0].name=="edited point"
        assert edited.geo_paths[0].points[1].z()==19 and edited.geo_paths[0].name=="edited path" and edited.geo_paths[0].surface=="draped"
        assert edited.compositions[0].name=="edited sheet" and edited.compositions[0].frames[0].scale_n==25 and edited.compositions[0].frames[0].x_mm==23
        assert edited.compositions[0].frames[0].uid=="persisted-frame-id"
        raw=json.loads((directory/"edited.igz").read_text())["scene"]
        assert raw["geo_points"][0]["opaque_point"]==43 and raw["geo_paths"][0]["opaque_path"]==47
        assert raw["compositions"][0]["opaque_sheet"]==59 and raw["compositions"][0]["frames"][0]["opaque_frame"]==53
        assert raw["opaque_document"]==document["scene"]["opaque_document"]
        assert raw["faces"][0]["opaque_face"]==document["scene"]["faces"][0]["opaque_face"]
        fresh=Scene();igz.load_into(fresh,directory/"fresh.igz")
        assert fresh.groups[0].name=="fresh Rust group"
        g=fresh.groups[0];assert (g.xform[0,3],g.xform[1,3],g.xform[2,3])==(4,5,6),"Fresh Rust transformed inline group ignored by original"
        assert len(g.mesh.faces)==1
        assert fresh.saved_views[0].name=="Rust saved view" and fresh.saved_views[0].fov_deg==61 and not fresh.saved_views[0].perspective
        assert fresh.saved_views[0].target==(2,3,4)
        plane=fresh.section_planes[0];assert plane.name=="Rust section" and plane.symbol=="Q" and plane.point.x()==3 and plane.normal.x()==1
        point=fresh.geo_points[0];assert point.name=="survey point" and point.desc=="description" and point.position.z()==13
        path=fresh.geo_paths[0];assert path.name=="survey path" and path.closed and path.surface=="flat" and path.points[2].y()==4
        sheet=fresh.compositions[0];assert sheet.name=="Rust composed sheet" and sheet.paper=="A4" and not sheet.landscape
        assert sheet.frames[0].scale_n==50 and sheet.frames[0].x_mm==31
        assert fresh.georef.lat==-16 and fresh.georef.lon==-71 and fresh.georef.alt==2400 and fresh.georef.north==12
    report={"provenance":context(),"passed":True,"checks":["edited loose geometry","edited persistent instance identity/name/transform","unknown face/document metadata retained","fresh Rust transformed group original runtime reopen","fresh Rust saved view, section plane and geographic datum original runtime interpretation","fresh geographic point/path and sheet scale/layout original runtime interpretation","edited geographic points/paths and sheet frame scale/position persistent identity interpreted by original"],"scope":"Synthetic geometry/group/view/section/datum/geographic/sheet fields; actual Python runtime loader, no GUI editing/render proof."}
    Path("docs/audits/python-edit-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
