#!/usr/bin/env python3
import os,sys,json,weakref,gc
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.image_plane import ImagePlane
from core.history import AddImagePlaneCommand
from views.viewport import Viewport
from acceptance_context import context
app=QApplication([]);v=Viewport();v.flash_status=lambda *a:None
im=ImagePlane("old",V(),V(1,0,0),V(0,1,0));assert gc.is_tracked(im);old_id=id(im);v.history.execute(AddImagePlaneCommand(im));del im;gc.collect()
states=[{"old_alive":any(id(x)==old_id for x in gc.get_objects()),"live":len(v.scene.image_planes),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}]
v.history.undo();gc.collect();states.append({"old_alive":any(id(x)==old_id for x in gc.get_objects()),"live":len(v.scene.image_planes),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)})
new=ImagePlane("new",V(),V(1,0,0),V(0,1,0));assert id(new)!=old_id;v.history.execute(AddImagePlaneCommand(new));gc.collect();states.append({"old_alive":any(id(x)==old_id for x in gc.get_objects()),"live":len(v.scene.image_planes),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)})
Path(sys.argv[1]).write_text(json.dumps({"provenance":context(),"states":states},indent=2)+"\n")
