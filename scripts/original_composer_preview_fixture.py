import sys,os,json,dataclasses
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtCore import QTimer,QPointF
from views.viewport import Viewport
from views.composer import ComposerWindow
from core.composition import Composicion
app=QApplication([]);host=QMainWindow();host.viewport=Viewport();comp=Composicion();comp.frames.clear();host.viewport.scene.compositions=[comp];window=ComposerWindow(host);window.resize(1200,800);window.show();out=Path(os.environ['INGETRAZO_PREVIEW_OUT'])
def point(p):
 p=window._view.mapFromScene(p);p=window._view.viewport().mapTo(window,p);return [p.x(),p.y()]
def state():
 button=window._draw_tb.widgetForAction(window._tool_actions['rect']);b=button.mapTo(window,button.rect().center());paper=comp.page_size_mm();a=point(QPointF(0,0));z=point(QPointF(*paper));start=window._view._drag_start
 value=dict(paper_mm=list(paper),tool=window.tool_mode,start=[start.x(),start.y()]if start is not None else None,items=[dataclasses.asdict(s)for s in comp.shapes],undo=len(window.history._undo),redo=len(window.history._redo),metrics=None,preview_alive=window._view._preview is not None,button=[b.x(),b.y()],paper=a+z)
 (out/'state.tmp').write_text(json.dumps(value));(out/'state.tmp').replace(out/'state.json')
timer=QTimer();timer.timeout.connect(state);timer.start(30);app.exec()
