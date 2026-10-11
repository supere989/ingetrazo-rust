#!/usr/bin/env python3
"""Owned two-process recovery acceptance. No user config or HOME mutation."""
import argparse,hashlib,json,os,queue,shutil,subprocess,threading,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('binary');p.add_argument('output');a=p.parse_args();binary=Path(a.binary).resolve();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=False);state=out/'state';rows=[];writer=None;reader=None;stderr=None;messages=queue.Queue(maxsize=4)
report={'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'source_sha':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'states':rows,'cleanup':[]}
def save(): (out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
def run(mode):
 r=subprocess.run([str(binary),mode,str(state)],capture_output=True,text=True,timeout=20);(out/(mode+'-'+str(len(rows))+'.stdout')).write_text(r.stdout);(out/(mode+'-'+str(len(rows))+'.stderr')).write_text(r.stderr);assert r.returncode==0,(mode,r.returncode,r.stderr);return json.loads(r.stdout)
def read_stdout():
 # Windows selectors do not support anonymous subprocess pipes. One owned
 # daemon reader and bounded protocol queue serve both readiness messages.
 try:
  while True:
   text=writer.stdout.readline(65537)
   if not text:messages.put(('eof',None),timeout=1);return
   if len(text)>65536:raise ValueError('Writer protocol line exceeds 64KiB')
   messages.put(('line',text),timeout=1)
 except BaseException as error:
  try:messages.put(('error',repr(error)),timeout=1)
  except queue.Full:pass

def line():
 try:kind,text=messages.get(timeout=10)
 except queue.Empty:raise AssertionError('Writer readiness timed out')
 assert kind=='line',('Writer readiness failed',kind,text)
 return json.loads(text)
def scan(name,expected):
 r=run('scan');rows.append({'phase':name,**r});save();assert r['count']==expected and r['show_recovery']==bool(expected) and r['error'] is None;return r
try:
 run('init');original=(state/'original.igz').read_bytes();report['original_sha256']=hashlib.sha256(original).hexdigest();shutil.copyfile(state/'original.igz',out/'original-before.igz')
 stderr=(out/'writer.stderr').open('w');writer=subprocess.Popen([str(binary),'writer',str(state)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,text=True);reader=threading.Thread(target=read_stdout,name="owned-recovery-stdout",daemon=True);reader.start();ready=line();report['writer']=ready;save();assert ready['ready'] and ready['pid']==writer.pid
 scan('live-independent-owner',0);slot=Path(ready['slot']);meta=slot.with_suffix('.json');raw=meta.read_bytes();meta.unlink();scan('live-without-metadata',0);meta.write_bytes(raw)
 writer.stdin.write('drop-app\n');writer.stdin.flush();assert line()['clone_holds_owner'];scan('clone-retains-lease',0)
 writer.kill();writer.wait(timeout=5);stderr.close();report['abrupt_exit']=writer.returncode;assert writer.returncode!=0;scan('crash-visible',1)
 record=json.loads(meta.read_text());record['owner_pid']=os.getpid();meta.write_text(json.dumps(record));scan('reused-live-pid-does-not-hide-crash',1)
 lease=state/'data/autosave'/('.owner-'+ready['session']+'.lock');lease.unlink();lease.mkdir();bad=run('scan');rows.append({'phase':'lease-observation-refusal',**bad});save();assert bad['count'] is None and bad['error'] and not bad['show_recovery'];lease.rmdir();lease.touch()
 recovered=run('recover');rows.append({'phase':'egui-picker-recover-close-save',**recovered});save();assert recovered['recovered']==[9,8,7] and recovered['saved_copy'];assert (state/'original.igz').read_bytes()==original
 shutil.copyfile(state/'original.igz',out/'original-after.igz');shutil.copyfile(state/'recovered.igz',out/'recovered.igz');report['original_unchanged']=True;report['complete']=True
except BaseException as e:report['failure']=repr(e);save();raise
finally:
 if writer is not None and writer.poll() is None:
  writer.terminate()
  try:writer.wait(timeout=5);report['cleanup'].append('owned writer terminated')
  except subprocess.TimeoutExpired:
   report['cleanup'].append('owned writer terminate timeout');writer.kill();writer.wait(timeout=5);report['cleanup'].append('owned writer killed')
 if reader is not None:
  reader.join(timeout=3)
  if reader.is_alive():report['cleanup'].append('owned stdout reader timeout')
 if writer is not None:
  if writer.stdin:writer.stdin.close()
  if writer.stdout and (reader is None or not reader.is_alive()):writer.stdout.close()
 if stderr is not None:stderr.close()
 save()
 if reader is not None and reader.is_alive():raise RuntimeError('Owned stdout reader did not terminate after process cleanup')
print(json.dumps({'complete':report.get('complete',False),'states':len(rows),'original_unchanged':report.get('original_unchanged',False)}))
