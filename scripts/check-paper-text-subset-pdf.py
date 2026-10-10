#!/usr/bin/env python3
"""Fresh bounded source helper/native subset PDF protocol, not app Text parity."""
import os,sys,subprocess,json,hashlib,shutil
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def run(a,expected=0,env=None):
 p=subprocess.run([str(x)for x in a],env=env);assert p.returncode==expected,(a,p.returncode,expected)
assert subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9','Original source pin changed'
original=out/'original';native=out/'native';env=dict(os.environ,INGETRAZO_TEXT_EXPANDED='1')
run([sys.executable,'scripts/probe-python-paper-text.py',original],env=env)
run([sys.executable,'scripts/check-paper-subset-paths.py',original,original/'font-observations.json',out/'intermediate'])
run(['cargo','build','--locked','-p','ingetrazo-formats','--example','paper_text_pdf_fixture'])
binary=Path(os.environ.get('CARGO_TARGET_DIR','target'))/'debug/examples/paper_text_pdf_fixture'
run([binary,original/'font-observations.json',native])
for dpi in [72,144,300]:run([sys.executable,'scripts/check-paper-text-pages.py',original,native,dpi])
negative=out/'negative';negative.mkdir(exist_ok=True)
for p in native.glob('*.native.pdf'):shutil.copy2(p,negative/p.name)
p=negative/'latin.native.pdf';b=p.read_bytes();a=b'0.000000000 -92.812500000 Td';assert b.count(a)==1;p.write_bytes(b.replace(a,b'1.000000000 -92.812500000 Td'))
run([sys.executable,'scripts/check-paper-text-pages.py',original,negative,72],expected=1)
report=json.loads((negative/'reader-72.json').read_text());assert not report['passed'];assert [r['case']for r in report['cases']if not r['passed']]==['latin']
remaining=['underline','wrap','newlines','center','right','arabic','hebrew','cjk','arabic_run','hebrew_run']
(out/'fresh-gate.json').write_text(json.dumps({'passed':True,'strict_pages':27,'intermediate_cases':9,'native_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'negative':'valid same-length first-glyph Td x+1 rejected','remaining_original_cases':remaining,'scope':'Experimental direct subset encoder with fixed source helper frame, not production app export/fontresolution/paragraph/cache/bidi/ToUnicode/UI/fullP29'},indent=2)+'\n')
