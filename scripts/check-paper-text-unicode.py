#!/usr/bin/env python3
"""Actual PDF reader semantics, including a same-size valid ToUnicode corruption control."""
import os,sys,json,re
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
app=QApplication([])
original,native=map(Path,sys.argv[1:3])
names=["accents","bold","combining","italic","kerns","latin","mono","serif","serif_bold"]
def read(path):
 doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;assert doc.pageCount()==1
 return doc.getAllText(0).text()
rows=[]
for name in names:
 a=read(original/(name+".original.pdf"));b=read(native/(name+".native.pdf"));rows.append(dict(case=name,original=a,native=b,passed=a==b))
assert all(r["passed"]for r in rows),rows
raw=(native/"latin.native.pdf").read_bytes();match=re.search(rb"<0001> <([0-9a-fA-F]{4})>",raw);assert match
value=b"0058"if match[1]!=b"0058"else b"0059"
changed=raw[:match.start(1)]+value+raw[match.end(1):];assert len(changed)==len(raw)
path=native/"unicode-mutated.pdf";path.write_bytes(changed)
assert read(path)!=read(native/"latin.native.pdf"),"Valid ToUnicode mutation must change reader text"
report=dict(passed=True,cases=rows,negative="same-size parse-valid ToUnicode mutation rejected",scope="Nine retained actual helper pages only; no paragraph/bidi/fallback/UI claim")
(native/"unicode-reader.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report))
