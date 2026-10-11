#!/usr/bin/env python3
"""Re-execute only the source Qt case against retained native captures."""
import sys,json,time,subprocess,shutil,os
from pathlib import Path
reference,out=map(Path,sys.argv[1:3]);case=sys.argv[3];assert case in['gondola_v20-iso','gondola_v20-asymmetric','capilla_quiroz_v17-iso','capilla_quiroz_v17-asymmetric'];assert not out.exists();out.mkdir(mode=0o700);(out/case).mkdir();shutil.copytree(reference/case/'native',out/case/'native')
meta=json.loads((reference/'transport.json').read_text());row=next(r for r in meta['runs']if r['label']==case+'-original');command=row['command'].copy();name='ingetrazo-annotation-source-'+str(os.getpid());command[command.index('--name')+1]=name
command[command.index(str(reference)+':/out:rw')]=str(out)+':/out:rw'
started=time.monotonic();record=dict(command=command,retained_native_reference=str(reference),timeout_seconds=60)
try:
 with(out/'source.log').open('w')as stream:record['returncode']=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=60).returncode
except subprocess.TimeoutExpired:record.update(returncode=124,timed_out=True)
finally:
 record['elapsed']=time.monotonic()-started
 try:record['cleanup_returncode']=subprocess.run(['podman','rm','--force',name],capture_output=True,timeout=10).returncode
 except subprocess.TimeoutExpired:record['cleanup_timed_out']=True
 (out/'transport.json').write_text(json.dumps(record,indent=2)+'\n')
raise SystemExit(record['returncode'])
