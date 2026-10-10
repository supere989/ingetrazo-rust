#!/usr/bin/env python3
"""Operative original Move array crossing and exact overlap behavior."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.move import MoveTool
from acceptance_context import context
app=QApplication([]);rows=[]
for overlap in (False,True):
 vp=Viewport();vp.flash_status=lambda text:None
 edge=vp.scene.mesh.add_edge(V(-1,0,0),V(1,0,0))
 if overlap:vp.scene.mesh.add_edge(V(-1,2,0),V(1,2,0))
 else:vp.scene.mesh.add_edge(V(0,1,0),V(0,5,0))
 vp.scene.select([edge]);tool=MoveTool();vp.set_active_tool(tool)
 tool.on_click(N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0)));tool._copy=True
 assert tool.on_value(vp,(0.,2.,0.));assert not vp.history.last_error
 assert tool.on_array_value(vp,2,'x');assert not vp.history.last_error
 def geometry():return sorted(sorted([list(e.a.toTuple()),list(e.b.toTuple())]) for e in vp.scene.mesh.edges)
 copied=geometry();undo_entries=len(vp.history.undo_stack);vp.history.undo();undone=geometry();vp.history.redo();redone=geometry()
 rows.append({'name':'exact overlap' if overlap else 'crossing vertical wire','accepted':True,'copied_segments':copied,'undo_segments':undone,'redo_segments':redone,'undo_entries':undo_entries,'redo_equal':copied==redone})
Path('docs/audits/move-array-crossing-source-controls.json').write_text(json.dumps({'provenance':context(),'cases':rows,'scope':'Actual original Move copy and array commands through History, controlled displacement. Crossings and exact overlap are accepted; raw undo and redo segment sets retained. Not rendered gesture proof.'},indent=2)+'\n')
print('Original crossing/overlap accepted; raw history segments retained')
