#!/usr/bin/env python3
"""Authored original entities, original IGZ codec; no production parser changes."""
from pathlib import Path
import sys,json,subprocess
source,out=map(Path,sys.argv[1:3]);out.mkdir(exist_ok=False)
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path.insert(0,str(source))
from core.scene import Scene
from core.textlabel import TextLabel
from core.dimension import Dimension
from PySide6.QtGui import QVector3D
from formats.igz import save_scene
s=Scene();s.text_labels=[TextLabel(QVector3D(-1,0,0),QVector3D(1,1,0),'Alpha circle'),TextLabel(QVector3D(1,-1,0),QVector3D(.7,.5,0),'Second circle')]
s.dimensions=[Dimension(QVector3D(-1,-1,0),QVector3D(1,-1,0),QVector3D(0,-.7,0),text='Alpha arrows')]
s.dimension_style=dict(s.dimension_style,color=[37,93,157,128],font_size=11)
save_scene(s,out/'scene.igz')
camera=dict(target=[0,0,0],distance=5,yaw=-.7853981633974483,pitch=.5235987755982988,up=[0,0,1],fov_deg=45,aspect=4/3,znear=.1,zfar=10000,perspective=False,two_point=False)
(out/'camera.json').write_text(json.dumps(camera)+'\n')
(out/'source-entities.json').write_text(json.dumps(dict(source_pin='6be29fe437a7cd992d4e079f71720821b25216b9',dimension_style=s.dimension_style,leaders=[x.to_dict()for x in s.text_labels]),indent=2)+'\n')
