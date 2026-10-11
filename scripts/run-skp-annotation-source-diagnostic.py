#!/usr/bin/env python3
"""Re-execute only the source Qt case against retained native captures."""
import sys,json,time,subprocess,shutil,os,hashlib
from pathlib import Path
reference,out=map(Path,sys.argv[1:3]);case=sys.argv[3];assert case in['gondola_v20-iso','gondola_v20-asymmetric','capilla_quiroz_v17-iso','capilla_quiroz_v17-asymmetric'];assert not out.exists();out.mkdir(mode=0o700);(out/case).mkdir();shutil.copytree(reference/case/'native',out/case/'native')
meta=json.loads((reference/'transport.json').read_text());row=next(r for r in meta['runs']if r['label']==case+'-original');command=row['command'].copy();name='ingetrazo-annotation-source-'+str(os.getpid());command[command.index('--name')+1]=name
command[command.index(str(reference)+':/out:rw')]=str(out)+':/out:rw'
tree=Path(__file__).resolve().parents[1];helper=tree/'scripts/reference-skp-application-annotations.py'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
provenance=dict(worktree=str(tree),worktree_commit=subprocess.check_output(['git','-C',str(tree),'rev-parse','HEAD'],text=True).strip(),diagnostic_script_sha256=digest(Path(__file__)),source_script_sha256=digest(helper),source_pin='6be29fe437a7cd992d4e079f71720821b25216b9',parser_pin='291700bc213655a00461838de71e5f206311e619',native_files={str(p.relative_to(out/case/'native')):digest(p)for p in sorted((out/case/'native').rglob('*'))if p.is_file()})
started=time.monotonic();record=dict(command=command,retained_native_reference=str(reference),timeout_seconds=60,execution_provenance=provenance)
try:
 with(out/'source.log').open('w')as stream:record['returncode']=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=60).returncode
except subprocess.TimeoutExpired:record.update(returncode=124,timed_out=True)
finally:
 record['elapsed']=time.monotonic()-started
 record['source_script_unchanged']=digest(helper)==provenance['source_script_sha256']
 record['diagnostic_script_unchanged']=digest(Path(__file__))==provenance['diagnostic_script_sha256']
 if not record['source_script_unchanged']or not record['diagnostic_script_unchanged']:record['returncode']=1
 try:record['cleanup_returncode']=subprocess.run(['podman','rm','--force',name],capture_output=True,timeout=10).returncode
 except subprocess.TimeoutExpired:record['cleanup_timed_out']=True
 (out/'transport.json').write_text(json.dumps(record,indent=2)+'\n')
raise SystemExit(record['returncode'])
