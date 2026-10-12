#!/usr/bin/env python3
"""Missing ancestor command target: actual survey then live nested context Undo."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QInputDialog
from views.viewport import Viewport
from views.tray import SurveyPointsPanel
from views.filedialogs import file_dialogs
from formats import igz
out=Path(sys.argv[1]).resolve();app=QApplication([]);win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;igz.load_into(s,out/'fresh-input.igz');panel=SurveyPointsPanel(win);rows=[]
def record(phase):rows.append(dict(case='ancestor-history',phase=phase,points=[p.to_dict() for p in s.geo_points],datum=s.georef.to_dict() if s.georef else None,units=s.units,undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),depth=len(s._edit_stack),count=panel._count.text(),camera=dict(target=list(vp.camera.target.toTuple()),distance=vp.camera.distance)))
file_dialogs.getOpenFileName=lambda *a,**k:(str(out/'comma.csv'),'');QInputDialog.getInt=lambda *a,**k:(18,True);QInputDialog.getItem=lambda *a,**k:('South',True)
record('before');panel._on_import();record('import');vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0]);record('enter');vp.history.undo();panel.refresh();record('undo');vp.history.redo();panel.refresh();record('redo');vp.end_group_edit();panel.refresh();record('finish');igz.save_scene(s,out/'ancestor-history-original.igz');vp.history.undo();panel.refresh();record('finish-undo');vp.history.redo();panel.refresh();record('finish-redo')
(out/'original-survey-context.json').write_text(json.dumps(dict(states=rows),indent=2)+'\n');print(json.dumps(dict(states=len(rows),actual_context_history=True)))
