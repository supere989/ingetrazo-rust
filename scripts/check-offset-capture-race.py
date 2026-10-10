#!/usr/bin/env python3
"""Deterministic actual eframe screenshot callback identity control, not Offset geometry parity."""
import os,json,subprocess,tempfile,select,hashlib,time,sys
from pathlib import Path
from PySide6.QtGui import QImage
root=Path(__file__).resolve().parents[1];binary=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples/offset_capture_race_fixture';out=Path(os.environ['INGETRAZO_CAPTURE_RACE_OUT']);out.mkdir(parents=True,exist_ok=True)
assert binary.is_file();rows=[]
for legacy in [False,True]:
 for mode in ['normal','interleaved','empty']:
  directory=out/(('old-'if legacy else'fixed-')+mode);directory.mkdir(exist_ok=True)
  assert not (directory/'runtime.log').exists(),'Use a fresh owned output directory'
  with tempfile.TemporaryDirectory(prefix='offset-capture-race-')as runtime:
   server=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','1200x900x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);app=None
   try:
    assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
    env=dict(os.environ,DISPLAY=':'+display,XDG_RUNTIME_DIR=runtime,LIBGL_ALWAYS_SOFTWARE='1',INGETRAZO_OFFSET_CAPTURE_MODE=mode);env.pop('WAYLAND_DISPLAY',None)
    if legacy:env['INGETRAZO_OFFSET_LEGACY']='1'
    else:env.pop('INGETRAZO_OFFSET_LEGACY',None)
    request=directory/'capture.tmp';request.write_text('A');request.replace(directory/'capture.txt')
    with(directory/'runtime.log').open('w')as log:
     app=subprocess.Popen([str(binary),str(directory.resolve())],env=env,stdout=log,stderr=log);code=app.wait(timeout=30)
    images={}
    for name in ['A','B']:
     path=directory/(name+'.png')
     if path.exists():
      image=QImage(str(path));assert not image.isNull()and image.width()>900 and image.height()>600
      images[name]={'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'size':[image.width(),image.height()]}
    log=(directory/'runtime.log').read_text();callbacks=json.loads((directory/'callbacks.json').read_text())if(directory/'callbacks.json').exists()else None
    if legacy and mode=='empty':assert code!=0 and 'format: Unknown'in log and 'Unsupported'in log
    elif legacy and mode=='interleaved':assert code==0 and set(images)=={'B'} and callbacks['callbacks']==2
    else:assert code==0 and set(images)==({'A','B'}if mode=='interleaved'else{'A'})and callbacks['callbacks']==(2 if mode=='interleaved'else 1)
    rows.append({'legacy':legacy,'mode':mode,'exit':code,'images':images,'callbacks':callbacks,'scope':'real production eframe screenshot callback; observer stimulus only; no injected image/event','negative_reproduced':legacy and mode!='normal'})
   finally:
    if app and app.poll()is None:app.terminate();app.wait(timeout=10)
    server.terminate();server.wait(timeout=10)
assert len(rows)==6 and sum(r['negative_reproduced']for r in rows)==2
(out/'report.json').write_text(json.dumps({'passed':True,'candidate':'e71bc48def2a1cbbb9658758393ac9ec0db6b4d4','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256((root/'crates/ingetrazo-gui/examples/offset_capture_race_fixture.rs').read_bytes()).hexdigest(),'checker_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'states':rows},indent=2)+'\n')
print('PASS fixed actual callbacks3controls; old empty panic and interleaved wrong identity2negatives')
