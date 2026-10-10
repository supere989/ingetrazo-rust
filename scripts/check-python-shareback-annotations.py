#!/usr/bin/env python3
"""Fresh pinned-original command oracle against freshly built native application."""
import json,subprocess,sys,tempfile,shutil,os
from contextlib import nullcontext
from pathlib import Path
from acceptance_context import context
requested=os.environ.get('INGETRAZO_SHAREBACK_EVIDENCE')
if requested:
 evidence=Path(requested)
 if evidence.exists():raise RuntimeError('Refusing existing shareback evidence directory')
 evidence.mkdir(parents=True)
with (nullcontext(requested) if requested else tempfile.TemporaryDirectory(prefix='ingetrazo-shareback-')) as tmp:
 root=Path(tmp)
 subprocess.run([sys.executable,'scripts/probe-python-shareback-annotations.py',str(root/'original.json')],check=True)
 subprocess.run(['cargo','run','--locked','-p','ingetrazo-gui','--example','shareback_annotation_fixture','--',str(root/'native')],check=True)
 shutil.copytree('docs/audits/shareback-annotations/old-snapshot',root/'old-snapshot')
 subprocess.run([sys.executable,'scripts/check-shareback-annotations.py',str(root)],check=True)
 report=json.loads((root/'result.json').read_text());report['provenance']=context()
 Path('docs/audits/python-shareback-annotations-current.json').write_text(json.dumps(report,indent=2)+'\n')
