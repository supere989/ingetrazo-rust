#!/usr/bin/env python3
"""Capture actual unchanged Qt-owned bridge sockets; no native expected outputs."""
import argparse,copy,hashlib,json,os,socket,subprocess,sys,time
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);args=ap.parse_args();out=args.output.resolve();assert not out.exists();out.mkdir(parents=True)
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
contract=json.loads((repo/'docs/audits/ai-bridge-python/fixture-contract.json').read_text())
for row in contract['original_sources']:assert hashlib.sha256((original/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
sys.path.insert(0,str(original));os.environ['INGETRAZO_AI_PORT']='0'
from PySide6.QtWidgets import QApplication
from views.main_window import MainWindow
from plugins.ai_bridge import _Bridge
app=QApplication.instance()or QApplication([]);host=MainWindow();vp=host.viewport
bridge=_Bridge(vp);port=bridge.start();client=socket.create_connection(('127.0.0.1',port),timeout=3);client.setblocking(False)
callback_errors=[];sys.excepthook=lambda typ,error,tb:callback_errors.append({'type':typ.__name__,'error':str(error)})
face="mesh.add_face([QVector3D(0,0,0),QVector3D(2,0,0),QVector3D(0,2,0)])"
recipes=[('fragmented-and-ordered-requests',[{'id':'fragment','tool':'run_python','args':{'code':"counter=6; held=mesh"}},{'id':'next','tool':'run_python','args':{'code':'counter'}}]),('stdout-stderr-eval-persistent-scope',[{'id':'streams','tool':'run_python','args':{'code':"import sys; print('hello'); sys.stderr.write('warning\n')"}},{'id':'unicode-write','tool':'run_python','args':{'code':"print(sys.stdout.write('α🙂')); sys.stderr.flush()"}},{'id':'eval','tool':'run_python','args':{'code':'counter+1'}},{'id':'underscore','tool':'run_python','args':{'code':'_+1'}}]),('geometry-history',[{'id':'add','tool':'run_python','args':{'code':face}},{'id':'undo','tool':'undo'},{'id':'redo','tool':'redo'}]),('noop-preserves-redo',[{'id':'undo2','tool':'undo'},{'id':'inspect','tool':'run_python','args':{'code':"print(len(mesh.faces))"}},{'id':'redo2','tool':'redo'}]),('mutation-error-rollback',[{'id':'undo-before-error','tool':'undo'},{'id':'rollback','tool':'run_python','args':{'code':"survivor=23\n"+face+"\nraise ValueError('literal-failure')"}},{'id':'scope-after-error','tool':'run_python','args':{'code':"assert held is mesh; print(survivor)"}},{'id':'redo-after-error','tool':'redo'}]),('syntax-type-dispatch-errors',[{'id':'syntax','tool':'run_python','args':{'code':'def broken('}},{'id':'type','tool':'run_python','args':{'code':42}},{'id':'unknown','tool':'not_a_tool'},None])]
report={'source_pin':PIN,'cases':[],'states':[],'callback_errors':callback_errors,'scope':'Actual loopback sockets and Qt queued UI dispatch; raw CPython traceback retained; no screenshot/pixel or native full Scene API claim'}
def observe():
 return {'faces':[{'outer':[[v.x(),v.y(),v.z()]for v in f.vertices],'holes':[[[v.x(),v.y(),v.z()]for v in h]for h in f.holes],'attrs':copy.deepcopy(f.attrs)}for f in vp.scene.mesh.faces],'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'groups':len(vp.scene.groups),'layers':len(vp.scene.layers)}
buffer=b''
def reply():
 global buffer
 end=time.monotonic()+10
 while b'\n'not in buffer:
  assert time.monotonic()<end,'Socket reply timeout'
  app.processEvents()
  try:chunk=client.recv(65536)
  except BlockingIOError:continue
  assert chunk,'Socket unexpectedly closed';buffer+=chunk
 line,buffer=buffer.split(b'\n',1);return json.loads(line)
try:
 for name,requests in recipes:
  report['cases'].append({'name':name,'requests':requests})
  # First pair is one real fragmented/coalesced stream, not two helper dispatches.
  if name=='fragmented-and-ordered-requests':
   data=b''.join((json.dumps(r)+'\n').encode()for r in requests);client.sendall(data[:7]);app.processEvents();client.sendall(data[7:])
  for index,request in enumerate(requests):
   if name!='fragmented-and-ordered-requests':client.sendall((json.dumps(request)+'\n').encode()if request is not None else b'{bad-json}\n')
   response=reply();report['states'].append({'case':name,'index':index,'reply':response,'scene':observe()})
   (out/'original-ai-bridge.partial.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
 assert callback_errors==[],callback_errors
 assert next(s for s in report['states'] if s['reply'].get('id')=='underscore')['reply']['result']['stdout']=='8\n'
 assert next(s for s in report['states'] if s['reply'].get('id')=='unicode-write')['reply']['result']['stdout']=='α🙂2\n'
 rollback=next(s for s in report['states']if s['reply'].get('id')=='rollback');assert rollback['reply']['ok']is True and rollback['reply']['result']['changed']is False and rollback['reply']['result']['error']=='SnapshotImport: literal-failure'
 (out/'original-ai-bridge-python.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
 print(json.dumps({'cases':len(report['cases']),'states':len(report['states']),'actual_socket':True,'callback_errors':callback_errors}))
finally:client.close();bridge.stop();host.hide()
