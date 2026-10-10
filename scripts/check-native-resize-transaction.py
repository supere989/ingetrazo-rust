#!/usr/bin/env python3
"""Require actual NVIDIA allocations, discarded-owner equality, retry and final pixels."""
import argparse,json,re
from pathlib import Path
from PySide6.QtGui import QImage
STAGES=["BeforeCreate","ImageView(1)","RenderPass","Framebuffer(1)","EguiPipeline","CadPipeline","Sync","BeforeCommit"]
def check(root):
 log=(root/"startup.log").read_text()
 assert 'NVIDIA GeForce RTX 5090' in log and 'enabled=true' in log,"Actual NVIDIA/present-fence device proof missing"
 observed=[]
 for line in log.splitlines():
  if line.startswith('RESIZE_CANCELLATION '):
   stage,detail=line.removeprefix('RESIZE_CANCELLATION ').split(' published_owners_preserved=',1)
   assert detail.startswith('true retired_retry=true audit=')
   audit=json.loads(detail.split('audit=',1)[1]);assert audit['created']==audit['destroyed'],f"Candidate leak at {stage}"
   observed.append(stage)
 assert observed==STAGES,f"Missing/reordered actual cancellation checkpoints: {observed}"
 assert 'RESIZE_TRANSACTION_ALL_8_PASS' in log
 assert 'Native bounded GPU cleanup complete; present_fences_enabled=true; committed_bytes=0' in log
 metrics=re.search(r'CAD2D metrics: Stats \{ ([^}]+)',log).group(1)
 assert 'uploads: 1,' in metrics,"Resize uploaded cached CAD geometry again"
 assert int(re.search(r'cache_hits: (\d+)',metrics).group(1))>=8
 for file,size in [('resize-800x600.png',(800,600)),('resize-1100x700.png',(1100,700))]:
  image=QImage(str(root/file));assert not image.isNull() and (image.width(),image.height())==size
  for x,y,rgba in [(60,60,(146,174,206,255)),(35,35,(0,0,0,255)),(140,40,(255,0,0,255)),(180,40,(0,255,0,255)),(140,80,(0,0,255,255))]:
   assert image.pixelColor(x,y).getRgb()==rgba,(file,x,y,image.pixelColor(x,y).getRgb(),rgba)
 return {'passed':True,'actual_checkpoints':observed,'cached_geometry_uploads':1,'tracked_ledger_shutdown':0}
def main():
 a=argparse.ArgumentParser();a.add_argument('root',type=Path);a.add_argument('--output',type=Path);a.add_argument('--old-cleanup',type=Path);a.add_argument('--old-pipeline',type=Path);args=a.parse_args()
 result=check(args.root)
 for label,path,completed in [('old-cleanup',args.old_cleanup,1),('old-pipeline',args.old_pipeline,4)]:
  if path is None:raise AssertionError('Both actual old-source negatives are mandatory')
  log=(path/'startup.log').read_text()
  assert 'NVIDIA GeForce RTX 5090' in log
  assert 'Resize cancellation replaced or destroyed published owners' in log
  assert log.count('RESIZE_CANCELLATION ')==completed
  assert 'RESIZE_TRANSACTION_ALL_8_PASS' not in log
  assert 'committed_bytes=0' in log
 result['actual_old_source_negatives_rejected']=['old-cleanup','old-pipeline']
 # Negative controls for claimed checkpoint/cleanup truth are mandatory.
 result['scope']='Engineering cancellation after real allocations, not native driver OOM/device-loss or total implicit VRAM accounting'
 if args.output:args.output.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result))
if __name__=='__main__':main()
