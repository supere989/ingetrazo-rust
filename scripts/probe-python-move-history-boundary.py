#!/usr/bin/env python3
"""Retained original stale-target counterexample; not a passing literal parity case."""
import sys,os,json
os.environ["QT_QPA_PLATFORM"]="offscreen";sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from types import SimpleNamespace as N
from views.viewport import Viewport
from views.main_window import MainWindow
from tools.move import MoveTool
from core.edits import build_add_edge
app=QApplication([]);vp=Viewport();vp.history.execute(build_add_edge(vp.scene,V(0,0,0),V(1,0,0)));edge=vp.scene.mesh.edges[0];vp.scene.select([edge]);t=MoveTool();vp.set_active_tool(t);t.on_click(N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0)));t.on_hover(N(viewport=vp,world=V(0,1,0),screen=QPointF(0,0)))
MainWindow._on_undo(N(viewport=vp));report={"edges_after_undo":len(vp.scene.mesh.edges),"active_move_anchor_retained":t.start_point is not None,"captured_targets_retained":len(t._positions),"scope":"Actual original MainWindow._on_undo with standalone viewport, controlled active Move preview. No pointer/UI capture."}
from pathlib import Path
from acceptance_context import context
report["provenance"]=context()
assert report["active_move_anchor_retained"] and report["captured_targets_retained"]==2
Path("docs/audits/move-active-history-original-boundary.json").write_text(json.dumps(report,indent=2)+"\n")
print("Original active Move retains detached targets after Undo: retained boundary")
