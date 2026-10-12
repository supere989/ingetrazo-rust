#!/usr/bin/env python3
"""Strict finite default figure/model/resource/original-reader comparison."""
import argparse,copy,hashlib,json,math,struct
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
NAMES=['default','disabled-string','bool-false']
FLAGS=['startup_clean','history_empty','owned_asset','raw_false_enabled','disabled_pristine_reconcile','new_document_preference','edited_startup_not_removed','loaded_document_marker_retired']
def equal(a,b,path='$'):
 if type(a) in (int,float) and type(b) in (int,float):
  assert math.isfinite(a) and math.isfinite(b) and a==b,path
  if a==0:assert math.copysign(1,a)==math.copysign(1,b),path
 elif isinstance(a,dict):
  assert type(b)is dict and a.keys()==b.keys(),path
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert type(b)is list and len(a)==len(b),path
  for i,(x,y) in enumerate(zip(a,b)):equal(x,y,path+f'[{i}]')
 else:assert type(a)is type(b) and a==b,path
 def_unused=None
def verify(s,n,reopens):
 assert s['source_pin']==n['source_pin']==PIN
 assert [c['name']for c in s['cases']]==[c['name']for c in n['cases']]==[c['name']for c in reopens['cases']]==NAMES
 assert s['callback_errors']==[]
 for a,b,pair in zip(s['cases'],n['cases'],reopens['cases']):
  for k in ['name','setting','state','clean','undo','redo','saved','reopen']:equal(a[k],b[k],a['name']+'.'+k)
  equal(a['reopen'],pair['original'],a['name']+'.original_reader');equal(pair['original'],pair['native'],a['name']+'.native_reader')
 for flag in FLAGS:assert n['controls'][flag]is True,flag
 equal(s['resources'],n['resources'],'resources')
def main():
 ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('native',type=Path);ap.add_argument('--report',type=Path);a=ap.parse_args()
 s=json.loads((a.source/'original-default-figure.json').read_text());n=json.loads((a.native/'native-default-figure.json').read_text());rr=json.loads((a.native/'reopens-default-figure.json').read_text());result={'ok':False,'cases':3,'readers':3,'negative_controls':[]}
 try:
  verify(s,n,rr)
  for key,row in s['resources'].items():
   for field,hashfield in [('file','encoded_sha256'),('rgba','rgba_sha256')]:
    x=(a.source/row[field]).read_bytes();y=(a.native/row[field]).read_bytes();assert x==y and hashlib.sha256(y).hexdigest()==row[hashfield],row[field]
  for label in ['inventory','component','geometry','resource','clean','history']+FLAGS:
   bad=copy.deepcopy(n)
   if label=='inventory':bad['cases'].pop()
   elif label=='component':bad['cases'][0]['state']['groups'][0]['component']=False
   elif label=='geometry':bad['cases'][0]['state']['groups'][0]['faces'][0]['outer'][0][0]+=1
   elif label=='resource':bad['resources'][next(iter(bad['resources']))]['rgba_sha256']='0'*64
   elif label=='clean':bad['cases'][0]['clean']=False
   elif label=='history':bad['cases'][0]['undo']=1
   else:bad['controls'][label]=False
   try:verify(s,bad,rr)
   except (AssertionError,KeyError,TypeError):result['negative_controls'].append(label)
   else:raise AssertionError('Accepted corruption '+label)
  result['ok']=True
 except Exception as e:result['error']=str(e)
 path=a.report or a.native/'comparison-default-figure.json';path.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));raise SystemExit(0 if result['ok']else 1)
if __name__=='__main__':main()
