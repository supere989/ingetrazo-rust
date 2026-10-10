#!/usr/bin/env python3
"""Actual unsubmitted Vulkan startup inventories, not driver OOM/loss injection."""
import argparse,json,re,copy
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('root',type=Path);a=p.parse_args()
s=(a.root/'positive'/'startup.log').read_text();r=(a.root/'refusal'/'startup.log').read_text()
rows=[(stage,json.loads(raw)) for stage,raw in re.findall(r'STARTUP_CANCEL (\w+) audit=(.*)',s)]
stages=['StartupInstance','StartupSurface','StartupDevice','StartupPool','StartupCommand','StartupSwapchain','StartupSync','StartupRenderer']
base={'Instance':1,'DebugMessenger':1};expected=[]
for stage in stages:
 if stage=='StartupSurface':base['Surface']=1
 if stage=='StartupDevice':base['Device']=1
 if stage=='StartupPool':base['CommandPool']=1
 if stage=='StartupCommand':base['CommandBuffer']=1
 if stage=='StartupSwapchain':base.update(Swapchain=1,ImageView=3,RenderPass=1,Framebuffer=3)
 if stage=='StartupSync':base.update(AcquireSemaphore=1,GraphicsFence=1,Semaphore=3,PresentFence=3)
 if stage=='StartupRenderer':base['EguiPipeline']=1
 expected.append((stage,base.copy()))
def verify(rows):
 assert len(rows)==8
 for (stage,audit),(wanted,inventory) in zip(rows,expected):
  assert stage==wanted
  assert audit['created']==audit['destroyed']==inventory,(stage,audit,inventory)
verify(rows)
assert 'STARTUP_ALL_8_PASS no_queue_submission=true' in s
assert 'Native Vulkan adapter: "NVIDIA GeForce RTX 5090"' in s and 'DISCRETE_GPU' in s
ref=json.loads(re.search(r'STARTUP_ACTUAL_ADAPTER_REFUSAL_PASS audit=(.*)',r).group(1))
assert ref['created']==ref['destroyed']=={'Instance':1,'DebugMessenger':1,'Surface':1}
neg=[]
for name,mutate in [('missing-stage',lambda x:x.pop()),('zero-inventory',lambda x:x[0][1]['created'].clear()),('missing-destruction',lambda x:x[-1][1]['destroyed'].pop('Swapchain'))]:
 bad=copy.deepcopy(rows);mutate(bad)
 try:verify(bad)
 except AssertionError:neg.append(name)
 else:raise AssertionError('Negative accepted '+name)
old=(a.root/'old-raw-negative'/'startup.log').read_text()
assert 'Unsubmitted startup owner leak:' in old and 'Swapchain: 1' in old
assert 'STARTUP_ALL_8_PASS' not in old
factory=(a.root/'factory-refusal'/'startup.log').read_text()
assert 'No suitable Vulkan adapter' in factory and 'panicked' not in factory
baseline=(a.root/'factory-refusal-baseline'/'startup.log').read_text()
assert 'panicked' in baseline and 'Option::unwrap()' in baseline
result={'status':'PASS','stages':rows,'actual_adapter_refusal':ref,'negative_controls':neg,'scope':'Actual RTX unsubmitted constructors only; typed device-loss policy unit test is not actual device loss/OOM/recovery proof.'}
(a.root/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
