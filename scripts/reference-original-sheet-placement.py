#!/usr/bin/env python3
"""Operative original composer placement/history; no algorithm substitutions."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from views.main_window import MainWindow
from views.composer import ComposerWindow
app=QApplication([]);main=MainWindow();composer=ComposerWindow(main);rows=[]
recipes=[("rect",[10,20,11,21]),("elipse",[40,30,10,5]),("poligono",[5,6,5,6]),("linea",[1,2,8,2]),("linea",[1,8,1,2]),("flecha",[1,9,8,2]),("flecha",[8,9,1,2]),("terreno",[2,3,8,7]),("terreno",[8,3,2,7])]
for kind,points in recipes:
 composer.tool_mode=kind;before=composer.comp.to_dict();composer.place_tool(*points);after=composer.comp.to_dict();shape=after["shapes"][-1]
 composer.history.undo();assert composer.comp.to_dict()==before
 composer.history.redo();assert composer.comp.to_dict()==after
 rows.append({"kind":kind,"points":points,"shape":shape,"undo_redo_exact":True})
out=Path("docs/audits/sheet-shape-original/placement-controls.json");out.write_text(json.dumps({"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","cases":rows,"boundary":"Actual original MainWindow/ComposerWindow place_tool and history; offscreen controller proof, no pointer or raster acceptance"},indent=2)+"\n")
print("Original placement/history9PASS")
