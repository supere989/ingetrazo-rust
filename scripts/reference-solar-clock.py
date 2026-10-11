#!/usr/bin/env python3
"""Frozen strict clock protocol; invoke the actual pinned source implementation."""
import argparse,datetime as dt,json,sys,subprocess,hashlib,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("--source",type=Path,required=True);p.add_argument("--output",type=Path,required=True);a=p.parse_args()
assert subprocess.check_output(["git","-C",str(a.source),"rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
sys.path.insert(0,str(a.source));from core import sun
from georef.datum import SceneDatum
cases=[]
def case(name,lat=-16.409,lon=-71.537,year=2026,month=3,day=21,hour=12,minute=0,second=0,offset=None,north=0.,enabled=True,valid_utc=None):
 settings=dict(enabled=enabled,month=month,day=day,hour=hour,minute=minute,darkness=.55)
 if offset is not None:settings["utc_offset"]=offset
 utc=valid_utc or dict(year=year,month=month,day=day,hour=hour,minute=minute,second=second)
 cases.append(dict(name=name,lat=lat,lon=lon,year=year,utc=utc,settings=settings,north=north))
for lat,lon in [(-90.,0.),(-66.5,151.2),(-16.409,-71.537),(0.,0.),(40.71,-74.),(66.5,25.7),(90.,0.)]:
 for month,day in [(3,20),(6,21),(9,22),(12,21)]:
  for hour in [0,5,6,12,18,23]:case(f"season-{lat}-{month}-{hour}",lat,lon,month=month,day=day,hour=hour,minute=7,second=31)
for lon in [-37.5,-22.5,-7.5,7.5,22.5,37.5]:case(f"half-even-{lon}",lon=lon)
for year,month,day,hour,offset in [(2024,2,29,0,1),(2026,1,1,0,23),(2026,12,31,23,-23),(1900,3,1,0,1),(2000,2,29,0,1),(2100,3,1,0,1)]:case(f"rollover-{year}-{month}",year=year,month=month,day=day,hour=hour,offset=offset)
for name,kw in [("nonleap",dict(month=2,day=29)),("short-month",dict(month=4,day=31)),("positive-offset",dict(offset=24)),("negative-offset",dict(offset=-24)),("huge-offset",dict(offset=2147483648)),("min-year-underflow",dict(year=1,month=1,day=1,hour=0,offset=1)),("max-year-overflow",dict(year=9999,month=12,day=31,hour=23,offset=-1))]:
 case(name,valid_utc=dict(year=2026,month=3,day=21,hour=12,minute=0,second=0),**kw)
for north in [-450.,-180.,-90.,30.,90.,180.,360.,450.]:case(f"north-{north}",north=north)
case("fallback-site",lat=40.,lon=25.,north=None)
case("disabled",enabled=False)
case("disabled-invalid",enabled=False,month=2,day=29,valid_utc=dict(year=2026,month=3,day=21,hour=12,minute=0,second=0))
def stamp(d):return {key:getattr(d,key) for key in ("year","month","day","hour","minute","second")}
expected=[]
for c in cases:
 lat,lon=c["lat"],c["lon"];s=sun.ShadowSettings.from_dict(c["settings"]);d=dt.datetime(**c["utc"])
 pos=sun.solar_position(lat,lon,d)
 try:utc=stamp(s.when_utc(lon,c["year"]));invalid=False
 except (ValueError,OverflowError):utc=None;invalid=True
 try:daylight=sun.daylight_minutes(lat,lon,s.month,s.day,s.utc_offset,c["year"]);invalid_daylight=False
 except (ValueError,OverflowError):daylight=None;invalid_daylight=True
 scene=None;invalid_scene=False
 if s.enabled:
  site=(lat,lon) if c["north"] is not None else (sun.DEFAULT_LAT,sun.DEFAULT_LON)
  try:
   scene=sun.sun_direction(*site,s.when_utc(site[1],c["year"]))
   if scene and c["north"] is not None:
    datum=SceneDatum(lat,lon,north=c["north"])
    if datum.north:scene=(*datum.grid_to_local_xy(*scene[:2]),scene[2])
  except (ValueError,OverflowError):invalid_scene=True
 expected.append(dict(position=dict(elevation=pos[0],azimuth=pos[1]),direction=sun.sun_direction(lat,lon,d),offset=sun.default_utc_offset(lon),utc=utc,invalid_local=invalid,daylight=daylight,invalid_daylight=invalid_daylight,scene_direction=scene,invalid_scene=invalid_scene))
a.output.mkdir(parents=True,exist_ok=True)
for name,value in [("input.json",cases),("original.json",expected)]: (a.output/name).write_text(json.dumps(value,indent=2)+"\n")
(a.output/"source.json").write_text(json.dumps(dict(commit="6be29fe437a7cd992d4e079f71720821b25216b9",files={name:hashlib.sha256((a.source/name).read_bytes()).hexdigest() for name in ["core/sun.py","georef/datum.py"]},cases=len(cases),acceptance="Exact numeric and discrete equality; no tolerances"),indent=2)+"\n")
print(f"Recorded {len(cases)} strict original solar clock cases")
