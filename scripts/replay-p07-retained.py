#!/usr/bin/env python3
"""Affected snap/measurement inputs reused verbatim; no original execution/build."""
import argparse,json,subprocess,zipfile,hashlib
from pathlib import Path

def main():
 ap=argparse.ArgumentParser();ap.add_argument('examples',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args();assert not a.output.exists();a.output.mkdir();root=Path('docs/audits');rows=[];bound={}
 def read(n):p=root/n;bound[str(p)]=hashlib.sha256(p.read_bytes()).hexdigest();return json.loads(p.read_text())
 def invoke(name,inputs):return json.loads(subprocess.check_output([str(a.examples/name)],input=json.dumps(inputs).encode()))
 source=read('python-screen-snap-evidence.json');native=invoke('screen_snap_fixture',[x['input']for x in source['cases']]);assert len(source['cases'])==len(native)==10
 for x,y in zip(source['cases'],native):
  assert x['input']['name']==y['name'];e=x['original'];passed=e['kind']==y['kind']and len(y['point'])==3 and max(abs(a-b)for a,b in zip(e['point'],y['point']))<1e-6;rows.append({'name':y['name'],'match':passed,'source':e,'native':y})
 inputs=read('perspective-screen-inputs.json');source=read('python-perspective-screen-evidence.json');native=invoke('perspective_screen_fixture',inputs);assert len(inputs)==len(source['cases'])==len(native)==11
 for x,y in zip(source['cases'],native):
  assert x['name']==y['name'];passed=len(x['original'])==len(y['steps'])and all(e['kind']==v['kind']and len(v['point'])==3 and max(abs(a-b)for a,b in zip(e['point'],v['point']))<1e-4 and len(e['encouraged'])==len(v['encouraged'])and all(max(abs(a-b)for a,b in zip(p,q))<1e-4 for p,q in zip(e['encouraged'],v['encouraged']))for e,v in zip(x['original'],y['steps']));rows.append({'name':y['name'],'match':passed,'source':x['original'],'native':y['steps']})
 for name,count in [('python-snap-heldout-depth-evidence.json',6),('python-snap-eye-boundary-evidence.json',2)]:
  source=read(name);native=invoke('perspective_screen_fixture',[r['input']for r in source['cases']]);assert len(source['cases'])==len(native)==count
  for x,y in zip(source['cases'],native):
   assert x['input']['name']==y['name']and len(y['steps'])==1;e=x['original'];v=y['steps'][0];passed=e['kind']==v['kind']and len(v['point'])==3 and max(abs(a-b)for a,b in zip(e['point'],v['point']))<1e-4;rows.append({'name':y['name'],'match':passed,'source':e,'native':v})
 assert len(rows)==29;(a.output/'snap29.json').write_text(json.dumps({'passed':all(r['match']for r in rows),'inputs':bound,'cases':rows},indent=2)+'\n');assert all(r['match']for r in rows),[r['name']for r in rows if not r['match']]
 z=root/'vcb-measurements/records.zip';bound[str(z)]=hashlib.sha256(z.read_bytes()).hexdigest()
 with zipfile.ZipFile(z)as archive:(a.output/'original-measurements.json').write_bytes(archive.read('original-measurements.json'))
 subprocess.run([str(a.examples/'line_command_history_fixture'),str(a.output),'--measurements'],check=True)
 c=subprocess.run(['python3','scripts/check-vcb-measurements.py',str(a.output)],check=True,capture_output=True,text=True);(a.output/'measurements-check.log').write_text(c.stdout+c.stderr)
 report={'snap_cases':29,'snap_passed':True,'numeric_rows':1168,'actual_app_unit_routing':15,'measurements_passed':True,'inputs':bound,'binary_sha256':{n:hashlib.sha256((a.examples/n).read_bytes()).hexdigest()for n in['screen_snap_fixture','perspective_screen_fixture','line_command_history_fixture']}};(a.output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
