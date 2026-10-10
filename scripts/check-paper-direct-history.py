#!/usr/bin/env python3
"""Actual original handlers/commands versus compiled native item-history contracts."""
import os,sys,json,subprocess,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
subprocess.run([sys.executable,'scripts/probe-python-direct-paper-history.py',str((out/'source.json').resolve())],cwd=root,check=True)
subprocess.run(['cargo','build','-q','-p','ingetrazo-gui','--example','paper_direct_history_acceptance_fixture'],cwd=root,check=True)
binary=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples/paper_direct_history_acceptance_fixture'
native=json.loads(subprocess.check_output([str(binary)],cwd=root,text=True));source=json.loads((out/'source.json').read_text());assert native['rows']==source['rows'],(native,source)
(out/'native.json').write_text(json.dumps(native,indent=2)+'\n');(out/'report.json').write_text(json.dumps({'passed':True,'exact_source_controls':len(source['rows']),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'scope':source['scope']},indent=2)+'\n');print('PASS 5 exact original/native paper property/history controls')
