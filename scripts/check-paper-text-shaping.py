#!/usr/bin/env python3
"""Frozen supplementary Qt glyph-run oracle. No native text/PDF closure claim."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
source=Path(sys.argv[1]);artifact=Path(sys.argv[2]);artifact.mkdir(parents=True,exist_ok=True)
original=json.loads(source.read_text())
for case in original["cases"]:
 for call in case["source_draw_calls"]:
  font=call["font"];assert hashlib.sha256(Path(font["file"]).read_bytes()).hexdigest()==font["sha256"],"sourcefont changed"
subprocess.run(["cargo","build","--locked","-p","ingetrazo-formats","--example","paper_text_fixture"],check=True)
binary=Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/paper_text_fixture"
p=subprocess.run([binary,source],capture_output=True,text=True);(artifact/"positive.log").write_text(p.stdout+p.stderr);assert p.returncode==0
negative=json.loads(source.read_text());negative["cases"][0]["source_draw_calls"][0]["supplementary_qtextlayout"]["runs"][0]["positions"][0][0]+=1/64
bad=artifact/"changed-position.json";bad.write_text(json.dumps(negative))
p=subprocess.run([binary,bad],capture_output=True,text=True);(artifact/"negative.log").write_text(p.stdout+p.stderr);assert p.returncode!=0 and "position latin" in p.stderr
(artifact/"summary.json").write_text(json.dumps({"passed":True,"source_sha256":hashlib.sha256(source.read_bytes()).hexdigest(),"native_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"exact_supplementary_qt_runs":10 if any(v["name"]=="combining" for v in original["cases"]) else 7,"negative":"1/64glyphposition perturbation rejected","scope":"Direct native fixedadvance shaping foundation only; no wrapping/bidi/fontfallback/PDF/UI parity"},indent=2)+"\n")
