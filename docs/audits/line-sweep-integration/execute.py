#!/usr/bin/env python3
"""One combined suite; original captures reused without re-execution."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parents[3]
work=Path('/tmp/ingetrazo-line-sweep-integration')
work.mkdir(exist_ok=False)
env=os.environ.copy()
env.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',XDG_CONFIG_HOME=str(work/'config'),XDG_CACHE_HOME=str(work/'cache'),QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo')
sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],cwd=root,text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert not subprocess.check_output(['git','-C','ingetrazo','status','--porcelain'],cwd=root,text=True).strip()
stages=[]
def run(name,cmd,output=None):
    start=time.monotonic()
    with (work/(name+'.log')).open('w') as log:
        if output is None:
            result=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
        else:
            with Path(output).open('w') as out:
                result=subprocess.run(cmd,cwd=root,env=env,stdout=out,stderr=log)
    stages.append(dict(name=name,command=cmd,source_commit=sha,exit=result.returncode,seconds=round(time.monotonic()-start,3)))
    (work/'stages.json').write_text(json.dumps(stages,indent=2)+'\n')
    print(name,result.returncode,flush=True)
    return result.returncode==0
run('workspace-tests',['cargo','test','--workspace','--locked','--tests','--no-fail-fast','-j2'])
run('doctests',['cargo','test','--workspace','--locked','--doc','--no-fail-fast','-j2'])
run('examples-check',['cargo','check','--workspace','--locked','--examples','-j2'])
names=['line_split_contract','drafting_acceptance_fixture','line_command_history_fixture','face_sweep_protocol']
cmd=['cargo','build','--locked','-j2','-p','ingetrazo-core','--example',names[0],'--example',names[1],'-p','ingetrazo-gui','--example',names[2],'-p','ingetrazo-render','--example',names[3]]
if not run('named-build',cmd):raise SystemExit('Named build failed; stale binaries will not execute')
binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples'
(work/'binary-sha256.json').write_text(json.dumps({n:hashlib.sha256((binary/n).read_bytes()).hexdigest() for n in names},indent=2)+'\n')
audit=root/'docs/audits'
split=audit/'line-orphan-history/split-contract/original.json'
if run('split-native',[str(binary/names[0]),str(split),str(work/'split-native.json')]):
    run('split-check',['python3','scripts/check-line-split-contract.py',str(split),str(work/'split-native.json'),str(work/'split-comparison.json')])
nine=work/'nine'
nine.mkdir()
original=audit/'line-orphan-history/raw-forward/commands'
for p in original.glob('*.igz'):shutil.copy2(p,nine/p.name)
shutil.copy2(original/'original.json',nine/'original.json')
if run('nine-native',[str(binary/names[2]),str(nine)],nine/'native.json'):
    run('nine-check',['python3','scripts/check-line-command-history.py',str(nine)])
frozen=json.loads((audit/'drafting-accepted-corpus.json').read_text())
assert len(frozen)==22
(work/'drafting-input.json').write_text(json.dumps([r['input'] for r in frozen]))
if run('drafting-native',[str(binary/names[1]),str(work/'drafting-input.json'),str(work/'drafting-native.json')]):
    checker="""import importlib.util,json,sys
from pathlib import Path
sys.path.insert(0,'scripts')
spec=importlib.util.spec_from_file_location('drafting_checker','scripts/check-python-drafting.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
frozen=json.loads(Path('docs/audits/drafting-accepted-corpus.json').read_text())
native=json.loads(Path(sys.argv[1]).read_text())
assert len(native)==len(frozen)==22
rows=[]
for f,n in zip(frozen,native):
 o=f['original']
 match=m.faces(o['faces'])==m.faces(n['faces']) and m.edges(o['edges'])==m.edges(n['edges']) and m.curves(o['edges'])==m.curves(n['edges'])
 rows.append({'case':f['input']['name'],'match':match})
Path(sys.argv[2]).write_text(json.dumps(rows,indent=2)+'\\n')
assert all(r['match'] for r in rows),[r for r in rows if not r['match']]
print('22 unchanged frozen drafting comparisons passed')
"""
    run('drafting-check',['/tmp/ingetrazo-parity-python/bin/python','-c',checker,str(work/'drafting-native.json'),str(work/'drafting-comparison.json')])
source=audit/'pushpull-reversed/source.json'
run('sweep-protocol',[str(binary/names[3]),str(source),str(work/'sweep-native.json')])
inputs=[split,source,audit/'drafting-accepted-corpus.json',original/'original.json']+list(original.glob('*.igz'))
(work/'input-sha256.json').write_text(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs},indent=2)+'\n')
raise SystemExit(any(s['exit'] for s in stages))
