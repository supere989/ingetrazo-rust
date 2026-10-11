#!/usr/bin/env python3
"""Exact original autosave write/pending/retire functions in owned storage."""
import hashlib,json,subprocess,sys
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtCore import qVersion
from PySide6.QtGui import QVector3D
from core.scene import Scene
from core import autosave
from formats import igz
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=False)
# Storage routing only. Original write/pending/clear and IGZ methods unchanged.
def directory():
 p=root/'autosave';p.mkdir(exist_ok=True);return p
autosave.autosave_dir=directory
scene=Scene();scene.mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])
original=root/'original.igz';igz.save_scene(scene,original);before=original.read_bytes();assert autosave.pending(original) is None
scene.mesh.vertices[0].position=QVector3D(9,8,7);slot=autosave.write(scene,original);assert autosave.pending(original)==slot
reopened=Scene();igz.load_into(reopened,slot);assert reopened.mesh.vertices[0].position==QVector3D(9,8,7)
autosave.clear(original);assert autosave.pending(original) is None;retired=list(autosave.discarded_dir().glob('*.igz'));assert len(retired)==1
saved=Scene();igz.load_into(saved,retired[0]);assert saved.mesh.vertices[0].position==QVector3D(9,8,7);assert original.read_bytes()==before
result={'source_pin':subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip(),'qt':qVersion(),'pending_before':False,'pending_after_write':True,'recovered':[9,8,7],'pending_after_retire':False,'discarded_copies':1,'discarded_recovered':[9,8,7],'original_unchanged':True,'source_sha256':hashlib.sha256(Path('/work/ingetrazo/core/autosave.py').read_bytes()).hexdigest(),'scope':'Actual original write/pending/clear and IGZ serialization. Storage function overridden only. Original single slot has no native lease or multi-window separation; no source lease parity claim.'}
(root/'original.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
