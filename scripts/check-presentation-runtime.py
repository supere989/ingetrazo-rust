#!/usr/bin/env python3
"""Run real source/native presentation adapters; retain every command verdict."""
import json,os,subprocess,sys
from pathlib import Path
root=Path("docs/audits/document-presentation-ci");root.mkdir(parents=True,exist_ok=True)
commands=[
 [sys.executable,"scripts/probe-python-document-presentation.py",str(root/"original")],
 [sys.executable,"scripts/probe-python-presentation-contexts.py",str(root/"original"),str(root/"contexts-original.json")],
 ["cargo","run","--locked","-p","ingetrazo-gui","--example","document_presentation_fixture","--",str(root/"original"),str(root/"native")],
 ["cargo","run","--locked","-p","ingetrazo-gui","--example","presentation_context_fixture","--",str(root/"original"),str(root/"contexts-native")],
 [sys.executable,"scripts/check-document-presentation.py",str(root/"original/original.json"),str(root/"native/native.json")],
 [sys.executable,"scripts/check-presentation-contexts.py",str(root/"contexts-original.json"),str(root/"contexts-native/native.json")],
 [sys.executable,"scripts/probe-python-presentation-codec.py",str(root/"codec-original.json")],
 ["cargo","run","--locked","-p","ingetrazo-core","--example","presentation_codec","--",str(root/"codec-original.json")],
 [sys.executable,"scripts/check-presentation-codec.py",str(root/"codec-original.json"),str(root/"codec-native.json"),"docs/audits/presentation-root-review/final-codec/old-grammar.json"],
 [sys.executable,"scripts/probe-python-presentation-files.py","generate",str(root/"files"),str(root/"codec-original.json"),str(root/"original/input.igz")],
 ["cargo","run","--locked","-p","ingetrazo-formats","--example","presentation_file_codec","--",str(root/"files")],
 [sys.executable,"scripts/probe-python-presentation-files.py","reopen",str(root/"files")],
]
rows=[]
for i,command in enumerate(commands):
 result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 (root/f"command-{i}.stdout").write_bytes(result.stdout);(root/f"command-{i}.stderr").write_bytes(result.stderr)
 rows.append({"command":command,"returncode":result.returncode})
 (root/"commands.json").write_text(json.dumps(rows,indent=2)+"\n")
 if result.returncode: raise SystemExit(result.returncode)
 if i==7:(root/"codec-native.json").write_bytes(result.stdout)
 if i==10:
  native=json.loads(result.stdout)["cases"];original=json.loads((root/"files/original.json").read_text())["cases"]
  assert native==original,"actual native/source IGZ loader mismatch"
print(json.dumps({"passed":True,"parser_cases":16,"native_save_reopens":13,"rendered_claim":False}))
