#!/usr/bin/env python3
"""Build actual HLR production source and compare fresh pinned Python outputs."""
import argparse,subprocess,sys,os,tempfile,hashlib,json,shutil
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("--source",type=Path,default=Path("."));p.add_argument("--original-python",default=sys.executable);p.add_argument("--output",type=Path,default=Path("docs/audits/composer-hlr-fresh-evidence.json"));a=p.parse_args();source=a.source.resolve();here=Path(__file__).resolve().parent
revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=source,text=True).strip()
subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","composer_hlr_fixture"],cwd=source,check=True)
target=Path(os.environ.get("CARGO_TARGET_DIR",source/"target"));exe=target/"debug/examples/composer_hlr_fixture"
with tempfile.TemporaryDirectory()as d:
 d=Path(d);snapshot=d/"native";shutil.copy2(exe,snapshot)
 subprocess.run([a.original_python,str(source/"scripts/reference-composer-hlr-fixture.py"),"--original",str(source/"ingetrazo"),"--output",str(d)],check=True)
 subprocess.run([str(snapshot),str(d)],check=True)
 subprocess.run([sys.executable,str(here/"check-composer-hlr-endpoints.py"),str(d),"--native-revision",revision,"--require-caps","--output",str(a.output)],check=True)
 r=json.loads(a.output.read_text());r["native_fixture_sha256"]=hashlib.sha256(snapshot.read_bytes()).hexdigest();r["input_sha256"]={f.name:hashlib.sha256(f.read_bytes()).hexdigest()for f in sorted(d.glob("*.igz"))};r["scope"]="Fresh locked production fixture build, fresh pinned original HLR, separate process actual original IGZ inputs; endpoints/classes in eight orthographic cases both hidden modes. Production cap triangles compared with original even-odd loop filled area and 83x79 deterministic point coverage including hollow cuts; shifted same-area cap negative control. Rendered widths/dashes remain pending separate acceptance.";a.output.write_text(json.dumps(r,indent=2)+"\n")
