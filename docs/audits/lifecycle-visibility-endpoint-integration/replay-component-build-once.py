#!/usr/bin/env python3
"""Replay only the changed component build orchestration; preserve prior stages."""
import os,json,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parents[3]
work=Path('/tmp/ingetrazo-lifecycle-visibility-integration')
env=os.environ.copy()
env.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',XDG_CONFIG_HOME=str(work/'config'),XDG_CACHE_HOME=str(work/'cache'),QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo')
stages=json.loads((work/'stages.json').read_text())
for name,args,expected in [
 ('component-build-once-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-python-components.py'],0),
 ('component-missing-prebuilt',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-python-components.py','--prebuilt-dir',str(work/'absent-binaries')],2),
]:
 start=time.monotonic()
 with (work/(name+'.log')).open('w') as log:
  result=subprocess.run(args,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 stages.append({'name':name,'command':args,'exit':result.returncode,'seconds':round(time.monotonic()-start,3)})
 (work/'stages.json').write_text(json.dumps(stages,indent=2))
 assert result.returncode==expected,(name,result.returncode)
assert 'Required prebuilt executable missing' in (work/'component-missing-prebuilt.log').read_text()
