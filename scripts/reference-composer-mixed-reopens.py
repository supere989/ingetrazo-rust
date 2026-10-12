#!/usr/bin/env python3
"""Actual unchanged original IGZ reader against native saved cohorts."""
import json,os,sys,subprocess
from pathlib import Path
source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve();original=Path('/work/ingetrazo');assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original));os.chdir(source)
from formats import igz
from core.scene import Scene
reference=json.loads((source/'original-mixed.json').read_text());report=json.loads((native/'native-mixed.json').read_text());assert [c['name']for c in reference['cases']]==[c['name']for c in report['cases']]
rows=[]
for case in report['cases']:
 path=Path(case['saved']);assert not path.is_absolute()and'..'not in path.parts
 scene=Scene();igz.load_into(scene,native/path);rows.append(dict(case=case['name'],compositions=[c.to_dict()for c in scene.compositions]))
(native/'native-reopens.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'readers':len(rows),'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9'}))
