#!/usr/bin/env python3
"""Capture actual original panel apply and refresh separately from GPU pixels."""
import os,sys,json,argparse,hashlib,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("--source",type=Path,required=True);p.add_argument("--output",type=Path,required=True);a=p.parse_args()
assert subprocess.check_output(["git","-C",str(a.source),"rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
os.environ["QT_QPA_PLATFORM"]="offscreen";sys.path.insert(0,str(a.source))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QDate,QSignalBlocker
q=QApplication([])
from views.tray import ShadowsPanel
from core.scene import Scene
from core.sun import ShadowSettings
from georef.datum import SceneDatum
class VP:
 def __init__(self):self.scene=Scene()
 def update(self):pass
class Win:
 def __init__(self):self.viewport=VP()
rows=[]
for name,lat,lon,month,day,minute,dark,offset in [("default",-16.409,-71.537,3,21,12*60,45,None),("afternoon",-16.409,-71.537,6,21,15*60+30,60,None),("zone-shift",-16.409,-71.537,3,21,12*60,45,-10),("polar-night",80.,0.,12,21,720,70,None),("polar-day",80.,0.,6,21,720,20,None),("jan-zone",40.71,-74.,1,1,1100,55,14),("june-zone",40.71,-74.,6,21,400,10,-12),("half-even",0.,22.5,9,22,720,80,None)]:
 w=Win();s=w.viewport.scene;s.georef=SceneDatum(lat,lon);s.shadows=ShadowSettings(enabled=False,hour=2,minute=3,darkness=.537,month=3,day=21);panel=ShadowsPanel(w)
 before=s.shadows.to_dict();panel.refresh();assert before==s.shadows.to_dict()
 blockers=[QSignalBlocker(v) for v in [panel._enable,panel._date,panel._time,panel._dark,panel._tz]]
 panel._enable.setChecked(True);panel._date.setDate(QDate(2026,month,day));panel._time.setValue(minute);panel._dark.setValue(dark);panel._tz.setCurrentIndex(panel._tz.findData(offset))
 controls=dict(enabled=panel._enable.isChecked(),month=panel._date.date().month(),day=panel._date.date().day(),hour=panel._time.value()//60,minute=panel._time.value()%60,darkness=1.-panel._dark.value()/100.)
 if panel._tz.currentData() is not None:controls["utc_offset"]=panel._tz.currentData()
 del blockers;panel._apply()
 rows.append(dict(name=name,lat=lat,lon=lon,before=before,controls=controls,after=s.shadows.to_dict(),range=[panel._time.minimum(),panel._time.maximum()]))
 panel.deleteLater()
a.output.write_text(json.dumps(dict(source="6be29fe437a7cd992d4e079f71720821b25216b9",panel_sha256=hashlib.sha256((a.source/"views/tray.py").read_bytes()).hexdigest(),rows=rows,day_of_year=[dict(day=d,month_day=ShadowsPanel._month_day_of(d)) for d in [1,31,32,59,60,80,181,365]]),indent=2)+"\n")
print("Recorded eight actual source panel apply states and read-only refreshes")
