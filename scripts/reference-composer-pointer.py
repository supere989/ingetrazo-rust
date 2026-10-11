#!/usr/bin/env python3
"""Missing-only actual Qt composer pointer/transaction source capture."""
import argparse,copy,hashlib,json,os,subprocess,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'ingetrazo')
from PySide6.QtWidgets import QApplication,QWidget
from PySide6.QtCore import Qt,QPointF,QEvent
from PySide6.QtGui import QMouseEvent,QKeyEvent,QTransform
from core.composition import Composicion,MarcoVista
from core.scene import Scene
from core.camera import OrbitCamera
from views.composer import ComposerWindow,FrameItem,ComposerCanvasView,_SheetItem
from PySide6.QtTest import QTest

class HostViewport:
    def __init__(self):self.scene=Scene();self.camera=OrbitCamera()
    def update(self):pass
class ProbeComposer(ComposerWindow):
    def render_frame(self,frame):pass # Exclude model rendering, retain actual canvas handlers.

p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();a.output.mkdir(parents=True)
app=QApplication([]);pin=subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9'
base={'uid':'base','x_mm':40.,'y_mm':40.,'w_mm':60.,'h_mm':30.,'z':0.,'locked':False,'hidden':False}
recipes=[]
for name in ['unselected-drag','selected-drag','large-first-move','click-only','locked','hidden','locked-overlay','hidden-overlay','z-order','equal-z-order','escape-during-drag','escape-before-move','out-and-back']:
 frames=[dict(base)];selected=[]
 if name=='selected-drag':selected=['base']
 if name in['locked','hidden']:frames[0][name]=True
 if name in['locked-overlay','hidden-overlay','z-order','equal-z-order']:
  frames.append(dict(base,uid='overlay',z=0. if name=='equal-z-order'else 10.,locked=name=='locked-overlay',hidden=name=='hidden-overlay'))
  if name=='z-order':frames.reverse()
 events=[{'kind':'press','at':[60.,55.]}]
 if name not in['click-only','escape-before-move']:
  if name!='large-first-move':events.append({'kind':'move','at':[64.,58.]})
  events.append({'kind':'move','at':[93.,86.]})
 if name=='out-and-back':events.append({'kind':'move','at':[60.,55.]})
 if name in['escape-during-drag','escape-before-move']:events.append({'kind':'escape'})
 events.append({'kind':'release','at':[60.,55.] if name in['click-only','out-and-back','escape-before-move']else[93.,86.]})
 recipes.append({'name':name,'frames':frames,'selected':selected,'guides_v':[],'events':events})
rows=[]
original_press=ComposerCanvasView.mousePressEvent
original_item_press=_SheetItem.mousePressEvent
trace=[]
def observed_press(self,event):
 trace.append({'handler':'view-press','spontaneous':event.spontaneous(),'mode':self.composer.tool_mode,'interactive':self.isInteractive(),'pos':[event.position().x(),event.position().y()],'items':[type(it).__name__ for it in self.items(event.position().toPoint())],'view_edit':self.composer.view_edit_item is not None})
 result=original_press(self,event)
 trace[-1 if trace[-1].get('handler')=='view-press'else -2]['accepted_after']=event.isAccepted()
 return result
def observed_item_press(self,event):
 trace.append({'handler':'item-press','uid':self.model.uid})
 return original_item_press(self,event)
ComposerCanvasView.mousePressEvent=observed_press
_SheetItem.mousePressEvent=observed_item_press
for recipe in recipes:
    host=QWidget();host.viewport=HostViewport();comp=ProbeComposer(host);comp._auto_render=False
    comp.comp.frames=[MarcoVista(**frame) for frame in recipe['frames']]
    for field in ['texts','images','scalebars','nortes','leyendas','shapes','cotas','cotas_ang','cotas_rad','etiquetas','perfiles','niveles','llamadas']:setattr(comp.comp,field,[])
    comp.comp.cajetin=None;comp.comp.guides_v=recipe['guides_v'];comp.comp.guides_h=[];comp.comp.paper='A4';comp.comp.landscape=True;comp.comp.margin_mm=10
    comp._rebuild_canvas();comp._set_tool_mode('select');comp.resize(1100,850);comp.show();app.processEvents();view=comp._view;view.setTransform(QTransform().scale(2.,2.));app.processEvents()
    for item in comp.canvas.items():
        if isinstance(item,FrameItem) and item.model.uid in recipe['selected']:item.setSelected(True)
    def state():
        selected=sorted(it.model.uid for it in comp.canvas.selectedItems() if isinstance(it,FrameItem))
        return {'frames':[{'uid':f.uid,'rect':[f.x_mm,f.y_mm,f.w_mm,f.h_mm],'locked':f.locked,'hidden':f.hidden,'z':f.z} for f in comp.comp.frames], 'selected':selected,'undo':len(comp.history._undo),'redo':len(comp.history._redo)}
    states=[{'phase':'before','state':state()}]
    for i,event in enumerate(recipe['events']):
        if event['kind']=='escape':
            view.keyPressEvent(QKeyEvent(QEvent.KeyPress,Qt.Key_Escape,Qt.NoModifier))
        else:
            pos=QPointF(view.mapFromScene(QPointF(*event['at'])))
            kind={'press':QEvent.MouseButtonPress,'move':QEvent.MouseMove,'release':QEvent.MouseButtonRelease}[event['kind']]
            button=Qt.NoButton if event['kind']=='move' else Qt.LeftButton
            held=Qt.NoButton if event['kind']=='release' else Qt.LeftButton
            mouse=QMouseEvent(kind,pos,pos,button,held,Qt.NoModifier)
            if event['kind']=='press':QTest.mousePress(view.viewport(),Qt.LeftButton,Qt.NoModifier,pos.toPoint())
            elif event['kind']=='release':QTest.mouseRelease(view.viewport(),Qt.LeftButton,Qt.NoModifier,pos.toPoint())
            else:QTest.mouseMove(view.viewport(),pos.toPoint())
            trace.append({'event':event['kind'],'mode':comp.tool_mode,'grabber':type(comp.canvas.mouseGrabberItem()).__name__,'items':[{'kind':type(it).__name__,'pos':[it.pos().x(),it.pos().y()],'visible':it.isVisible(),'flags':str(it.flags())} for it in comp.canvas.items()if isinstance(it,FrameItem)]})
        app.processEvents();states.append({'phase':str(i)+'-'+event['kind'],'state':state()})
    comp.history.undo();app.processEvents();states.append({'phase':'undo','state':state()})
    comp.history.redo();app.processEvents();states.append({'phase':'redo','state':state()})
    rows.append({'recipe':recipe,'states':states});(a.output/'source.partial.json').write_text(json.dumps(rows,indent=2)+'\n')
    comp.hide();host.hide();comp.deleteLater();host.deleteLater();app.processEvents()
report={'trace':trace,'source_files':{f:hashlib.sha256((Path('/work/ingetrazo')/f).read_bytes()).hexdigest()for f in ['views/composer.py','core/composition.py']},'source_pin':pin,'cases':rows,'scope':'Actual QtTest viewport press/move/release into unchanged ComposerWindow/CanvasView/FrameItem input and ComposerHistory, frame model rendering suppressed only. Paper mm and selection/history recorded; no rendered pixels claimed. Escape-during-drag source result retained literally.'}
(a.output/'source.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'cases':len(rows),'phases':sum(len(r['states']) for r in rows),'after':[{'case':r['recipe']['name'],'state':r['states'][-3]['state']} for r in rows]}))
