#!/usr/bin/env python3
"""Compare original offset arrangement geometry, retaining split/collapse cases."""
import json,os,sys,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D
from core.offset import offset_regions,offset_chain
def point(p):return tuple(round(float(x),5) for x in p)
def ring(r):
    p=[point(x) for x in r]
    return min(tuple(p[i:]+p[:i]) for i in range(len(p))) if p else ()
def canonical(regions):return sorted((ring(o),tuple(sorted(ring(h) for h in holes))) for o,holes in regions)
def xyz(v):return [v.x(),v.y(),v.z()]
def main():
    square=[[0,0,0],[4,0,0],[4,4,0],[0,4,0]]
    neck=[[0,0,0],[2,0,0],[2,.8,0],[4,.8,0],[4,0,0],[6,0,0],[6,2,0],[4,2,0],[4,1.2,0],[2,1.2,0],[2,2,0],[0,2,0]]
    cases=[]
    for name,points,normal,d in [("inward",square,[0,0,1],.5),("outward",square,[0,0,1],-.5),("collapse",square,[0,0,1],2),("overshoot",square,[0,0,1],3),("neck-split",neck,[0,0,1],.3),("clockwise",square[::-1],[0,0,-1],.5),("vertical",[[x,0,y] for x,y,z in square],[0,-1,0],.5)]:
        cases.append(dict(name=name,points=points,normal=normal,distance=d,chain=False))
    for d in [.5,-.5]:
        cases.append(dict(name="chain"+str(d),points=[[0,0,0],[2,0,0],[2,2,0]],normal=[0,0,1],distance=d,chain=True))
    with tempfile.TemporaryDirectory() as t:
        inp,out=Path(t)/"in.json",Path(t)/"out.json";inp.write_text(json.dumps(cases))
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-core","--example","offset_acceptance_fixture","--",str(inp),str(out)],check=True)
        results=json.loads(out.read_text());rows=[]
        for c,r in zip(cases,results):
            p=[QVector3D(*p) for p in c["points"]];n=QVector3D(*c["normal"])
            if c["chain"]:
                original=offset_chain(p,n,c["distance"]);expected=[point(xyz(x)) for x in original];actual=[point(x) for x in r["chain"]]
            else:
                original=offset_regions(p,n,c["distance"]);expected=canonical([([xyz(x) for x in o],[[xyz(x) for x in h] for h in holes]) for o,holes in original]);actual=canonical([(x["outer"],x["holes"]) for x in r["regions"]])
            assert actual==expected,(c["name"],expected,actual)
            rows.append({"case":c["name"],"regions_or_points":len(actual)})
        assert results[0]["regions"]!=results[1]["regions"],"sign negative control"
        assert len(results[4]["regions"])==2,"split fixture must split"
    report={"provenance":context(),"passed":True,"cases":rows,"negative_controls_detected":2,"scope":"Original ordered ring/holes and open-chain coordinate parity; interactive edge picking, paint and history remain separate."}
    Path("docs/audits/python-offset-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
