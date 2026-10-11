#!/usr/bin/env python3
"""One pinned Guide codec control; selection corpus is reused unchanged."""
import json,sys,os
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from core.guide import Guide
from core.scene import Scene
from formats import igz
p=Path(sys.argv[2]);app=QApplication([])
if sys.argv[1]=='input':
 data=json.loads((p/'input.igz').read_text());data['scene']['guides']=[{'point':[3,2,0],'origin':[1,2,3],'vendor':{'opaque':[False,17,'kept']}},{'point':[0,3,0],'direction':[2,0,0]}]
 (p/'guide-input.igz').write_text(json.dumps(data))
else:
 expected=[{'point':[7.,8.,9.],'origin':[1.,2.,3.]},{'point':[0.,3.,0.],'direction':[1.,0.,0.]}]
 edited=Scene();fresh=Scene();igz.load_into(edited,p/'guide-edited.igz');igz.load_into(fresh,p/'guide-fresh.igz')
 observed=[g.to_dict() for g in edited.guides];assert observed==expected,(observed,expected)
 assert [g.to_dict() for g in fresh.guides]==[{'point':[4.,5.,6.],'origin':[0.,0.,0.]}]
 (p/'guide-original-reopen.json').write_text(json.dumps({'edited':observed,'fresh':[g.to_dict() for g in fresh.guides]},indent=2))
 print(json.dumps({'guide_reopens':2,'source_exact':True}))
