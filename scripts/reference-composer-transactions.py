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
from views.composer import ComposerWindow,FrameItem

class HostViewport:
    def __init__(self):self.scene=Scene();self.camera=OrbitCamera()
    def update(self):pass
class ProbeComposer(ComposerWindow):
    def render_frame(self,frame):pass # Exclude model rendering, retain actual canvas handlers.

p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();a.output.mkdir(parents=True)
app=QApplication([]);pin=subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9'
base={'uid':'base','x_mm':50.,'y_mm':55.,'w_mm':80.,'h_mm':50.,'z':0.,'locked':False,'hidden':False}
recipes=[]
for name in ['unselected-drag','selected-drag','click-only','locked','hidden','locked-overlay','hidden-overlay','z-order','page-snap','guide-snap','sibling-snap','escape-during-drag']:
    frames=[dict(base)];selected=[];guides=[];end=[91.,84.]
    if name=='selected-drag':selected=['base']
    if name in ['locked','hidden']:frames[0][name]=True
    if name in ['locked-overlay','hidden-overlay','z-order']:
        frames.append(dict(base,uid='overlay',z=10.,locked=name=='locked-overlay',hidden=name=='hidden-overlay'))
        if name=='z-order':frames.reverse() # Native array order alone is not source stacking.
    if name=='page-snap':end=[26.,84.] # Proposed x11 snaps to margin10.
    if name=='guide-snap':guides=[77.];end=[91.,84.]
    if name=='sibling-snap':frames.append(dict(base,uid='sibling',x_mm=156.,y_mm=140.,w_mm=40.,h_mm=20.))
    events=[{'kind':'press','at':[65.,70.]}]
    if name!='click-only':events += [{'kind':'move','at':[67.,71.]},{'kind':'move','at':end}]
    if name=='escape-during-drag':events.append({'kind':'escape'})
    events.append({'kind':'release','at':[65.,70.] if name=='click-only' else end})
    recipes.append({'name':name,'frames':frames,'selected':selected,'guides_v':guides,'events':events})
rows=[]
for recipe in recipes:
    host=QWidget();host.viewport=HostViewport();comp=ProbeComposer(host);comp._auto_render=False
    comp.comp.frames=[MarcoVista(**frame) for frame in recipe['frames']]
    for field in ['texts','images','scalebars','nortes','leyendas','shapes','cotas','cotas_ang','cotas_rad','etiquetas','perfiles','niveles','llamadas']:setattr(comp.comp,field,[])
    comp.comp.cajetin=None;comp.comp.guides_v=recipe['guides_v'];comp.comp.guides_h=[];comp.comp.paper='A4';comp.comp.landscape=True;comp.comp.margin_mm=10
    comp._rebuild_canvas();comp.resize(1100,850);comp.show();app.processEvents();view=comp._view;view.setTransform(QTransform().scale(2.,2.));app.processEvents()
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
            QApplication.sendEvent(view.viewport(),mouse)
        app.processEvents();states.append({'phase':str(i)+'-'+event['kind'],'state':state()})
    comp.history.undo();app.processEvents();states.append({'phase':'undo','state':state()})
    comp.history.redo();app.processEvents();states.append({'phase':'redo','state':state()})
    rows.append({'recipe':recipe,'states':states});(a.output/'source.partial.json').write_text(json.dumps(rows,indent=2)+'\n')
    comp.hide();host.hide();comp.deleteLater();host.deleteLater();app.processEvents()
report={'source_pin':pin,'cases':rows,'scope':'Actual Qt ComposerWindow/CanvasView/FrameItem input and ComposerHistory, frame model rendering suppressed only. Paper mm and selection/history recorded; no rendered pixels claimed. Escape-during-drag source result retained literally.'}
(a.output/'source.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'cases':len(rows),'phases':sum(len(r['states']) for r in rows),'after':[{'case':r['recipe']['name'],'state':r['states'][-3]['state']} for r in rows]}))
