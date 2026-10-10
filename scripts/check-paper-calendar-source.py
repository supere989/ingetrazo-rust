#!/usr/bin/env python3
"""Original field_values local-date provider versus native libc across midnight and DST.
Owned subprocess TZ only; never changes host timezone or wall clock.
"""
import json,os,sys,subprocess,hashlib
from pathlib import Path
from datetime import datetime,timezone
from zoneinfo import ZoneInfo
ROOT=Path(__file__).resolve().parents[1]
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
zones=['UTC','America/New_York','Europe/Berlin','Pacific/Auckland']
instants=['2026-01-01T00:00:00','2026-03-08T07:00:00','2026-11-01T06:00:00','2026-03-29T01:00:00','2026-10-25T01:00:00','2026-04-04T14:00:00','2026-09-26T14:00:00','2024-02-29T00:00:00']
timestamps=[int(datetime.fromisoformat(s).replace(tzinfo=timezone.utc).timestamp())+delta for s in instants for delta in [-1,0,1]]
# The source imports datetime inside field_values. Control only date.today in the owned source subprocess; its timestamp-to-local-date conversion remains native.
code="""import sys,time,json,datetime
sys.path.insert(0,str(__import__('pathlib').Path('ingetrazo').resolve()))
from core import composition
time.tzset()
RealDate=datetime.date
class ClockDate(RealDate):
 @classmethod
 def today(cls):return RealDate.fromtimestamp(current)
datetime.date=ClockDate
rows=[]
for t in json.loads(sys.argv[1]):
 current=t
 rows.append({'timestamp':t,'source_date':composition.field_values()['fecha']})
print(json.dumps(rows))"""
protocol={'source_pin':subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'zones':{},'scope':'Original composition.field_values date.today with controlled timestamps and native datetime local-date conversion; native Unix localtime_r. Windows/macOS live execution remains separate.'}
for zone in zones:
 env={**os.environ,'TZ':zone,'QT_QPA_PLATFORM':'offscreen'}
 local_midnights=[int(datetime(2026,month,day,tzinfo=ZoneInfo(zone)).timestamp())+delta for month,day in [(1,1),(3,8),(11,1)] for delta in [-1,0,1]]
 rows=json.loads(subprocess.check_output([sys.executable,'-c',code,json.dumps(timestamps+local_midnights)],cwd=ROOT,env=env,text=True))
 protocol['zones'][zone]=rows
path=out/'source.json';path.write_text(json.dumps(protocol,indent=2)+'\n')
for zone in zones:
 env={**os.environ,'TZ':zone,'INGETRAZO_CALENDAR_SOURCE':str(path.resolve())}
 # Use complete module path, avoiding a zero-test false positive.
 result=subprocess.run(['cargo','test','-q','-p','ingetrazo-gui','--lib','composer_text_fields::canvas_expansion_cache_tests::native_calendar_source_timestamp_protocol','--','--exact'],cwd=ROOT,env=env,capture_output=True,text=True)
 (out/(zone.replace('/','_')+'.log')).write_text(result.stdout+result.stderr)
 assert result.returncode==0 and '1 passed' in result.stdout,(zone,result.stdout,result.stderr)
(out/'report.json').write_text(json.dumps({'passed':True,'zones':zones,'source_timestamp_controls':sum(map(len,protocol['zones'].values())),'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'scope':protocol['scope']},indent=2)+'\n')
print('PASS',sum(map(len,protocol['zones'].values())),'native/original local calendar controls')
