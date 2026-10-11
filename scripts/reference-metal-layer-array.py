#!/usr/bin/env python3
"""Actual original layer checkbox controller + Move array history, no GUI gesture claim."""
import sys,pathlib,json,types,hashlib
sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication,QTreeWidgetItem
from PySide6.QtCore import Qt
from core.scene import Scene
from core.layers import Layer
from tools.move import MoveTool
from views.tray import LayersPanel
from tests.test_move_copy import _Vp,_ctx,_square,_xs
app=QApplication.instance() or QApplication([])
scene=Scene();face=_square(scene);scene.selection=[face];scene.layers.append(Layer('visible'))
vp=_Vp(scene);tool=MoveTool();tool.on_click(_ctx(vp,0,0));tool.on_key(vp,Qt.Key_Control,Qt.NoModifier);tool.on_click(_ctx(vp,6,0))
assert _xs(scene)==[0.,6.];command=vp.history.undo_stack[-1];version=scene.version
panel=types.SimpleNamespace(_updating=False,_scene=lambda:scene,_prune_selection=lambda _:None)
panel._window=types.SimpleNamespace(viewport=vp);panel._touch=lambda:LayersPanel._touch(panel)
item=QTreeWidgetItem(['visible','','']);item.setData(0,Qt.UserRole,'visible');item.setCheckState(1,Qt.Unchecked);item.setCheckState(2,Qt.Unchecked)
LayersPanel._on_item_changed(panel,item,1)
assert not scene.layer('visible').visible and scene.version>version and vp.history.undo_stack[-1] is command
assert tool.on_array_value(vp,3,'x');assert _xs(scene)==[0.,6.,12.,18.] and len(vp.history.undo_stack)==1
report={'passed':True,'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','source_controller':'LayersPanel._on_item_changed + _touch','before_copy':[0,6],'after_array':[0,6,12,18],'undo_count':1,'layer_history_command_unchanged':True,'scope':'Original actual Qt checkbox controller method and Move array commands; not delivered UI/pixel proof'}
print(json.dumps(report,indent=2))
