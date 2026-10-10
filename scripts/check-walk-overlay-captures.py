#!/usr/bin/env python3
"""Replay mandatory actual source/native Walk overlays and actual old negative."""
import json,sys
from pathlib import Path
from PySide6.QtGui import QImage
from png_capture import valid_png
from walk_overlay_pixels import inspect_walk_overlay
root,old=map(Path,sys.argv[1:]);rows=[]
for name in ['navigation-walk-final-native','navigation-walk-final-original','navigation-walk-final-native-cancel','navigation-walk-final-original-cancel']:
 p=root/name;report=json.loads((p/'report.json').read_text());state=json.loads((p/'walking-state.json').read_text());assert report['passed']and report['cancel']==name.endswith('cancel')
 for f in ['before.png','walking.png','cancelled.png'if report['cancel']else'released.png']:assert valid_png(p/f),'missing/unreadable '+name+'/'+f
 baseline=QImage(str(p/'before.png'));walking=QImage(str(p/'walking.png'));rows.append(dict(name=name,pixels=inspect_walk_overlay(walking,baseline,state['viewport'],state['walk_anchor'])))
 cleanup=json.loads((p/'cleanup.json').read_text());assert all(not v['kill_timeout']for v in cleanup.values())
 for v in [report['walking'],report['final'],report['settled']]:assert v['undo']==0 and v['version']==report['initial']['version']
state=json.loads((old/'walking-state.json').read_text())
try:inspect_walk_overlay(QImage(str(old/'walking.png')),QImage(str(old/'before.png')),state['viewport'],state['walk_anchor'])
except AssertionError as error:negative=str(error)
else:raise AssertionError('actual compiled old overlay failure admitted')
print(json.dumps(dict(passed=True,mandatory_cases=rows,actual_old_negative_rejected=negative,scope='actual owned-window Walk cross/ring, release/Escape controls; no whole-AA/timing/hardware claim')))
