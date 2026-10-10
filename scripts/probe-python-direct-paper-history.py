#!/usr/bin/env python3
"""Frozen source operative page/border handlers and item-history ordering."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QCheckBox,QDoubleSpinBox,QComboBox
from core.composition import Composicion,TextoItem,ComposerHistory,AddItemCommand,EditItemCommand
from views.composer import ComposerWindow
app=QApplication([]);comp=Composicion();history=ComposerHistory();dirty=[]
toggle=QCheckBox();width=QDoubleSpinBox();width.setValue(.5);radius=QDoubleSpinBox();styles=QComboBox();styles.addItem('single','single');paper=QComboBox();paper.addItems(['A4','A1']);landscape=QCheckBox()
owner=SimpleNamespace(comp=comp,_updating=False,border_check=toggle,border_mm=width,border_radius=radius,border_style=styles,paper_combo=paper,landscape_check=landscape,_mark_dirty=lambda:dirty.append(True),_rebuild_canvas=lambda:None)
def snapshot(label):return {'label':label,'undo':len(history._undo),'redo':len(history._redo),'positions':[[x.x_mm,x.y_mm]for x in comp.texts],'paper':comp.paper,'landscape':comp.landscape,'border':{'on':comp.border,'color':comp.border_color,'radius_mm':comp.border_radius_mm}}
rows=[];comp.border_color='#ffffff';ComposerWindow._on_border_changed(owner);history.undo();history.redo();rows.append(snapshot('empty_property_history'))
item=TextoItem(text='A',x_mm=20,y_mm=30);history.execute(AddItemCommand(comp,item));history.execute(EditItemCommand(item,{'x_mm':33.,'y_mm':44.}));toggle.setChecked(True);radius.setValue(5);ComposerWindow._on_border_changed(owner);paper.setCurrentText('A1');ComposerWindow._on_page_changed(owner)
history.undo();rows.append(snapshot('undo_item_preserves_properties'));comp.border_color='#ff0000';toggle.setChecked(False);ComposerWindow._on_border_changed(owner);history.redo();rows.append(snapshot('direct_property_preserves_redo'));history.undo();history.undo();rows.append(snapshot('undo_all_items'));history.redo();history.redo();rows.append(snapshot('redo_all_items'))
assert rows[0]['undo']==rows[0]['redo']==0
assert rows[1]['positions']==[[20,30]]and rows[1]['paper']=='A1'and rows[1]['border']['on']
assert rows[2]['positions']==[[33,44]]and rows[2]['border']['color']=='#ff0000'and not rows[2]['border']['on']
assert not rows[3]['positions']and rows[4]['positions']==[[33,44]]
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','rows':rows,'scope':'Operative source handlers on genuine source widget values and actual ComposerHistory commands; pointer entry proven separately'},indent=2)+'\n');print('PASS 5 source direct-property/item-history controls')
