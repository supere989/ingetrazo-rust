#!/usr/bin/env python3
"""Bounded own-process transport; unchanged native positive-time acceptance."""
import hashlib,json,pathlib,subprocess,sys,time
binary=pathlib.Path(sys.argv[1]).resolve();out=pathlib.Path(sys.argv[2]).resolve()
out.mkdir(parents=True,exist_ok=True)
command=[str(binary),str(out)];started=time.monotonic();timed_out=False
with (out/'presentation.log').open('w') as log:
    try:
        process=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=25)
        code=process.returncode
    except subprocess.TimeoutExpired:
        timed_out=True;code=None
transport={'command':command,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
 'source_revision':sys.argv[3],'elapsed_seconds':time.monotonic()-started,
 'timed_out':timed_out,'returncode':code,'physical_mac_claim':False}
(out/'transport.json').write_text(json.dumps(transport,indent=2)+'\n')
assert not timed_out and code==0, transport
native=json.loads((out/'result.json').read_text())
assert native['passed'] is True and native['displayed']>=6 and native['exact_resize'] is True
assert native['pending']==0 and native['submitted']==native['retired']
assert native['displayed']==native['retired']-native['skipped']
assert native['elapsed_seconds']<=15 and native['physical_mac_claim'] is False
print(json.dumps({'strict_native_reference_pass':True,'native':native},indent=2))
