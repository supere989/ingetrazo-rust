#!/usr/bin/env python3
"""Independent, fail-closed untouched IGZ preservation acceptance.
Compares recursive source semantics and all archive assets, not entity counts.
Added Rust defaults are allowed; every original key/value must survive.
"""
import argparse, copy, hashlib, json, math, subprocess, tempfile, zipfile
from pathlib import Path
EXPECTED = ("arco-yanque.igz", "banca-pergola-yanque.igz", "luminaria-solar-yanque.igz", "pileta-fuente-yanque.igz")
def read(path):
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            data = json.loads(archive.read("document.json"))
            assets = {n: hashlib.sha256(archive.read(n)).hexdigest() for n in archive.namelist() if not n.endswith("/") and n != "document.json"}
    else:
        data = json.loads(path.read_text()); assets = {}
    return data.get("scene", data), assets

def differences(a, b, path="$", errors=None):
    errors = [] if errors is None else errors
    if isinstance(a, dict):
        if not isinstance(b, dict): errors.append(path + ": object changed type"); return errors
        for key, value in a.items():
            if key not in b: errors.append(path + "." + key + ": missing")
            else: differences(value, b[key], path + "." + key, errors)
    elif isinstance(a, list):
        if not isinstance(b, list) or len(a) != len(b): errors.append(path + ": list length/type changed"); return errors
        for i, (x,y) in enumerate(zip(a,b)): differences(x,y,path+f"[{i}]",errors)
    elif isinstance(a, (float,int)) and not isinstance(a,bool):
        if not isinstance(b,(float,int)) or isinstance(b,bool) or not math.isclose(a,b,rel_tol=1e-10,abs_tol=1e-10): errors.append(path + ": number changed")
    elif type(a) is not type(b) or a != b: errors.append(path + ": value changed")
    return errors

def controls():
    source = {"groups":[{"transform":[1,2,3],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"holes":[],"material":"brick"}]}],"saved_views":[{"camera":{"eye":[4,5,6]}}]}
    assert not differences(source,copy.deepcopy(source))
    mutations = []
    for field in ("saved_views","groups"):
        changed=copy.deepcopy(source);del changed[field];mutations.append(changed)
    changed=copy.deepcopy(source);changed["groups"][0]["faces"][0]["vertices"][1][0]=2;mutations.append(changed)
    changed=copy.deepcopy(source);changed["groups"][0]["transform"][0]=9;mutations.append(changed)
    changed=copy.deepcopy(source);changed["groups"][0]["faces"][0]["material"]="wrong";mutations.append(changed)
    assert all(differences(source,m) for m in mutations)
    return len(mutations)

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--binary",default="target/debug/ingetrazo");parser.add_argument("--output",default="docs/audits/document-semantic-evidence.json");args=parser.parse_args()
    report={"negative_controls_detected":controls(),"fixtures":[],"scope":"Untouched recursive source field and archive asset preservation. Does not prove editable prototype semantics, Python reopen or GUI workflows."}
    with tempfile.TemporaryDirectory() as tmp:
        for name in EXPECTED:
            source=Path("ingetrazo/examples")/name
            row={"name":name,"errors":[]}
            if not source.is_file(): row["errors"]=["Required pinned original fixture missing; initialize recursive submodules"]
            else:
                dest=Path(tmp)/name
                proc=subprocess.run([args.binary,"convert",str(source),str(dest)],capture_output=True,text=True)
                row["conversion_exit_code"]=proc.returncode
                if proc.returncode: row["errors"]=[proc.stdout+proc.stderr]
                else:
                    a,aa=read(source);b,ba=read(dest)
                    row["errors"]=differences(a,b)
                    row["asset_errors"]=[n for n,h in aa.items() if ba.get(n)!=h]
                    row["source_asset_count"]=len(aa)
                    if row["asset_errors"]:row["errors"].append("Archive assets missing or changed")
            report["fixtures"].append(row)
    report["passed"]=all(not r["errors"] for r in report["fixtures"])
    output=Path(args.output);output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"passed":report["passed"],"fixtures": [{"name":r["name"],"differences":len(r["errors"])} for r in report["fixtures"]],"negative_controls_detected":report["negative_controls_detected"]},indent=2))
    return 0 if report["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
