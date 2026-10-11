#!/usr/bin/env python3
"""Missing Select hover only; consumes authenticated acquisition inputs, never recaptures Move."""
import os,sys,json,math
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF,QEvent,Qt
from views.viewport import Viewport
from tools.select import SelectTool
from core.group import Group
from core.mesh import Mesh,Edge
from formats import igz
app=QApplication([]);out=Path(sys.argv[1]);reference=json.loads((out/'acquisition.json').read_text());rows=[]
# Preserve thirty accepted raw hover controls; derive their per-target ink from
# the pinned source draw predicate and original observed hover identity.
old=json.loads((out/'first40-source.json').read_text())['states']
for row in old:
 if row['case'] in ['glyph','dimension']:continue
 row.pop('ink')
 hit=row['annotation_hover']
 row['inks']={'label':None if row['case']=='hidden' else [243,115,41] if hit==['label',0] else [45,55,75], 'dimension':[243,115,41] if hit==['dimension',0] else [45,55,75]}
 rows.append(row)
cases=['glyph','dimension','leader','edge-priority','group-priority','hidden','locked','nested']
for case in ["glyph","dimension"]:
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);vp.resize(800,600);igz.load_into(vp.scene,out/'input.igz');s=vp.scene
 if case=='nested':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
 lookup='leader-miss' if case=='leader' else 'dimension' if case in ['edge-priority','group-priority'] else case
 p=list(next(x['screen']for x in reference['states']if x['case']==lookup))
 if case in ['dimension','edge-priority','group-priority']:
  pa,pb=[vp._world_to_pixel(x)for x in s.dimensions[0].line_points()];p=[pa[i]*.8+pb[i]*.2 for i in range(2)]
 if case=='leader':
  pa=vp._world_to_pixel(s.text_labels[0].anchor);pb=vp._world_to_pixel(s.text_labels[0].position());p=[(pa[i]+pb[i])*.5 for i in range(2)]
 layer=next(x for x in s.layers if x.name=='named');layer.visible=case!='hidden';layer.locked=case=='locked'
 if case=='edge-priority':s.mesh.add_edge(V(-1,1,0),V(1,1,0))
 if case=='group-priority':
  mesh=Mesh();mesh.add_face([V(-1,.5,.1),V(1,.5,.1),V(1,1.5,.1),V(-1,1.5,.1)]);s.groups.append(Group(mesh,'hover-occluder'))
 s.dimension_style['color']=[11,22,33]
 s.select([s.dimensions[0] if case=='glyph' else s.text_labels[0]])
 tool=SelectTool();vp.active_tool=tool;tool.on_activate(vp)
 def record(phase):
  h=vp._hover_entity
  if phase in ["hover","same"] and case=="edge-priority":assert isinstance(h,Edge)
  if phase in ["hover","same"] and case=="group-priority":assert isinstance(h,Group)
  target=next((['label',i]for i,t in enumerate(s.text_labels)if t is h),None)
  if target is None:target=next((['dimension',i]for i,t in enumerate(s.dimensions)if t is h),None)
  rows.append(dict(case=case,phase=phase,screen=p,annotation_hover=target,inks={name:([243,115,41]if entity in s.selection or entity is h else list(s.dimension_style.get('color',[45,55,75])))if s.entity_visible(entity)else None for name,entity in [('label',s.text_labels[0]),('dimension',s.dimensions[0])]},selection=len(s.selection),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),depth=len(s._edit_stack),same_identity=(phase!='same' or h is getattr(vp,'_previous_observed_hover',h))))
  vp._previous_observed_hover=h
 def hover(screen):tool.on_hover(SimpleNamespace(viewport=vp,screen=QPointF(*screen)))
 record('before');hover(p);record('hover');hover(p);record('same');hover([790,590]);record('empty');hover(p);tool.on_deactivate(vp);record('retire')
 tool.on_activate(vp);tool.on_click(SimpleNamespace(viewport=vp,screen=QPointF(790,590),modifiers=Qt.NoModifier));record('clear-selection')
order={(c,q):i for i,(c,q)in enumerate((c,q)for c in cases for q in (['before','hover','same','empty','retire','clear-selection']if c in ['glyph','dimension']else ['before','hover','same','empty','retire']))};rows.sort(key=lambda r:order[(r['case'],r['phase'])]);(out/'original-hover.json').write_text(json.dumps(dict(states=rows),indent=2))
