import importlib.util, pathlib, zipfile, json, sys, copy
root=pathlib.Path(__file__).resolve().parents[4]
spec=importlib.util.spec_from_file_location('checker',root/'scripts/check-metal-world-material.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
z=zipfile.ZipFile(sys.argv[1])
class Node:
 def __init__(self,name='',changes=None):self.name=name;self.changes={} if changes is None else changes
 def __truediv__(self,key):return Node(self.name+'/'+key if self.name else key,self.changes)
 def read_bytes(self):return self.changes.get(self.name,z.read('native/'+self.name))
 def read_text(self):return self.read_bytes().decode()
source=root/'docs/audits/metal-world-scene/material-source';actual=m.check(source,Node());r=json.loads(z.read('native/report.json'));f=r['frames'][0];raw=z.read('native/'+f['name']+'.rgba');x,y=map(round,f['red_pixel']);at=(y*1280+x)*4;pixel=bytearray(raw);pixel[at]^=1
cases={'one-pixel':{f['name']+'.rgba':bytes(pixel)},'blank':{f['name']+'.rgba':bytes(len(raw))},'stale-back':{'opaque-painted-back.rgba':raw}}
for name,change in [('missing-warm',lambda q:q['frames'].pop(1)),('uncleared-ledger',lambda q:q.update(ledger_reserved=1)),('heap-owner-budget',lambda q:q['frames'][18]['sampled_heaps'][0].__setitem__(0,0))]:
 q=copy.deepcopy(r);change(q);cases[name]={'report.json':json.dumps(q).encode()}
negative={}
for name,changes in cases.items():
 try:m.check(source,Node(changes=changes))
 except AssertionError:negative[name]='rejected'
 else:raise RuntimeError('accepted mutation '+name)
print(json.dumps({'actual':actual,'negative_controls':negative},indent=2))
