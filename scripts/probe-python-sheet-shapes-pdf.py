#!/usr/bin/env python3
"""Frozen original actual ComposerWindow atlas export for literal shape corpus."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from views.viewport import Viewport
from views.composer import ComposerWindow
from core.composition import Composicion,FormaItem
app=QApplication([]);host=QMainWindow();host.viewport=Viewport()
recipes=json.loads(Path(sys.argv[1]).read_text());assert len(recipes)==9
comps=[]
for i,recipe in enumerate(recipes):
 comp=Composicion(name=str(i),paper="A4",landscape=False);comp.frames.clear();comp.shapes=[FormaItem(**recipe)];comps.append(comp)
host.viewport.scene.compositions=comps;window=ComposerWindow(host)
assert window.export_all_pdf(sys.argv[2])==[]
window.close();print("PASS actual original9page shape PDF atlas")
