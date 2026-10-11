"""Independent rational oriented-boundary integration; no Mesh.volume/kernel imports."""
import copy,hashlib,json,math,struct,sys,zipfile
from collections import defaultdict
from decimal import Decimal,localcontext
from fractions import Fraction as F
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
REQUIRED=['endpoint-free-wire','open-neighbor','paint-first-rim','oblique-cap','concave','turned-squashed']
HASHES={'fillet-oblique':{'source-fixed.json':'cd04f87be62db6b0bd066c1402bcd9e007f1531c62d5fed65099ac4b965084d4','native.json':'a0710b6a17306d57041221c3d2af1902244d515fd68d14f2a3489a7e05338295'},'fillet-nested-history':{'source.json':'8eff9f683b07ad4103f5f5d38b34dc108288b7937c83534a849b4a93d7376263','native.json':'337f89dd874bc08451d0e403dcf157316d9e3d02add240594a2945354bdc59ca'}}
def load(root,folder,name):
 with zipfile.ZipFile(root/folder/'records.zip')as z:raw=z.read(name)
 assert hashlib.sha256(raw).hexdigest()==HASHES[folder][name]
 return json.loads(raw)
def components(mesh):
 vertices=mesh['vertices'];faces=mesh['faces'];incidence=defaultdict(list)
 assert all(len(p)==3 and all(isinstance(x,(int,float))and not isinstance(x,bool)and math.isfinite(x)for x in p)for p in vertices)
 for i,face in enumerate(faces):
  for ring in [face['loop'],*face['holes']]:
   assert len(ring)>=3 and len(set(ring))==len(ring)
   for a,b in zip(ring,ring[1:]+ring[:1]):
    assert 0<=a<len(vertices)and 0<=b<len(vertices)and a!=b
    incidence[min(a,b),max(a,b)].append((i,a,b))
 adjacent=[set()for _ in faces]
 for edges in incidence.values():
  for i,_,_ in edges:adjacent[i].update(j for j,_,_ in edges if i!=j)
 unseen=set(range(len(faces)));closed=[];open_faces=[]
 while unseen:
  seed=min(unseen);unseen.remove(seed);found={seed};queue=[seed]
  while queue:
   i=queue.pop()
   for j in adjacent[i]&unseen:unseen.remove(j);found.add(j);queue.append(j)
  edges=[items for items in incidence.values()if items[0][0]in found]
  if all(len(e)==2 and e[0][1:]==e[1][1:][::-1]for e in edges):closed.append(sorted(found))
  else:open_faces.extend(sorted(found))
 return closed,open_faces

def add(a,b):return(a[0]+b[0],a[1]+b[1])
def neg(a):return(-a[1],-a[0])
def mul(a,b):
 v=[x*y for x in a for y in b];return(min(v),max(v))
def scalar(v):return(F(v),F(v))
def rounding_interval(x,bits):
 x=float(x)
 if bits==64:return(F(math.nextafter(x,-math.inf))+F(x))/2,(F(math.nextafter(x,math.inf))+F(x))/2
 q=struct.unpack('f',struct.pack('f',x))[0];assert q==x,'source coordinate is not Qt-f32'
 if q==0:return(-F(2)**-150,F(2)**-150)
 raw=struct.unpack('I',struct.pack('f',q))[0]
 prev=struct.unpack('f',struct.pack('I',raw-1 if q>0 else raw+1))[0]
 nxt=struct.unpack('f',struct.pack('I',raw+1 if q>0 else raw-1))[0]
 return((F(prev)+F(q))/2,(F(nxt)+F(q))/2)
def integral(mesh,face_ids,bits=None):
 origin=mesh['vertices'][mesh['faces'][face_ids[0]]['loop'][0]]
 # A fixed exact origin removes translation cancellation without altering
 # any admitted coordinate rounding cell or the closed-boundary integral.
 ps=[[add(rounding_interval(x,bits)if bits else scalar(x),scalar(-F(origin[k])))for k,x in enumerate(p)]for p in mesh['vertices']];total=(F(0),F(0))
 for fid in face_ids:
  face=mesh['faces'][fid]
  # Interior is preserved metadata, not a license to drop a boundary polygon.
  for ring in [face['loop'],*face['holes']]:
   a=ps[ring[0]]
   for i in range(1,len(ring)-1):
    b,c=ps[ring[i]],ps[ring[i+1]]
    for k in range(3):total=add(total,mul(a[k],add(mul(b[(k+1)%3],c[(k+2)%3]),neg(mul(b[(k+2)%3],c[(k+1)%3])))))
 return(total[0]/6,total[1]/6)
def volumes(mesh,bits=None):
 closed,opened=components(mesh);assert len(closed)==1,'requires exactly one oriented closed body'
 value=integral(mesh,closed[0],bits);assert value[0]>0,'outward positive body required';return value,len(opened)
def validate(source,native):
 sv,so=volumes(source);nv,no=volumes(native);assert so==no
 bound,_=volumes(source,32);assert bound[0]<=nv[0]<=bound[1],'volume outside source Qt rounding cell integral'
 return {'source_volume':float(sv[0]),'native_volume':float(nv[0]),'source_exact_fraction':str(sv[0]),'native_exact_fraction':str(nv[0]),'source_qt_rounding_interval':[float(v)for v in bound],'open_faces_excluded':so}
def analytic(radius,segments,length,box_volume):
 with localcontext()as ctx:
  ctx.prec=90
  def atan(x):
   term=x;total=x;k=1
   while abs(term)>Decimal(10)**-85:
    term*=-x*x;total+=term/Decimal(2*k+1);k+=1
   return total
  pi=16*atan(Decimal(1)/5)-4*atan(Decimal(1)/239)
  x=pi/(2*segments);term=x;s=x;k=1
  while abs(term)>Decimal(10)**-85:
   term*=-x*x/(Decimal(2*k)*Decimal(2*k+1));s+=term;k+=1
  return Decimal.from_float(box_volume)-Decimal.from_float(length)*Decimal.from_float(radius)**2*(1-Decimal(segments)*s/2)
def main():
 root=Path(sys.argv[1]);s=load(root,'fillet-nested-history','source.json');n=load(root,'fillet-nested-history','native.json');fixed=load(root,'fillet-oblique','source-fixed.json');fn=load(root,'fillet-oblique','native.json')
 assert s['source_pin']==fixed['source_pin']==PIN
 assert [c['name']for c in s['cases']]==[c['name']for c in n['cases']]==REQUIRED
 reference={c['name']:c['after']for c in s['cases']};models={c['name']:c for c in s['cases']}
 for c in fixed['cases']:reference[c['name']]=c['model']['after'];models[c['name']]=c['model']
 rows=[]
 for c in n['cases']:rows.append({'name':c['name'],**validate(reference[c['name']],c['after'])})
 for c in fn['cases']:validate(reference[c['name']],c['model']['after'])
 simple=next(c for c in n['cases']if c['name']=='endpoint-free-wire');model=models['endpoint-free-wire'];assert model['radius']==.2 and model['segments']==8
 before=model['before'];closed,_=components(before);body_vertices={v for fid in closed[0]for v in before['faces'][fid]['loop']}
 dimensions=[max(before['vertices'][v][k]for v in body_vertices)-min(before['vertices'][v][k]for v in body_vertices)for k in range(3)]
 assert dimensions==[2.,1.,1.]and len(closed[0])==6
 selected=before['edges'][model['selected'][0]]['ends'];edge_length=math.dist(*(before['vertices'][v]for v in selected));assert edge_length==2.
 box_volume=math.prod(dimensions)
 expected=F(analytic(model['radius'],model['segments'],edge_length,box_volume));native_interval,_=volumes(simple['after'],64)
 assert native_interval[0]<=expected<=native_interval[1],'discrete quarter-round analytic volume'
 negatives=[]
 for name in ['topology_preserving_scale','topology_preserving_vertex_displacement','reversed_boundary','removed_face','invalid_old_oblique']:
  m=copy.deepcopy(simple['after']);source=reference['endpoint-free-wire']
  if name=='topology_preserving_scale':
   m['vertices']=[[x*1.001 for x in p]for p in m['vertices']]
  elif name=='topology_preserving_vertex_displacement':m['vertices'][12][1]+=.01
  elif name=='reversed_boundary':m['faces'][0]['loop'].reverse()
  elif name=='removed_face':m['faces'].pop()
  else:source=next(c for c in s['cases']if c['name']=='oblique-cap')['after'];m=next(c for c in n['cases']if c['name']=='oblique-cap')['after']
  try:validate(source,m)
  except AssertionError:negatives.append(name)
  else:raise AssertionError('accepted '+name)
 report={'passed':True,'source_pin':PIN,'method':'Exact rational oriented polygon boundary determinant; Qt binary32 rounding-cell interval propagation, no fitted volume percentage. Analytic discrete quarter-circle corner checked against binary64 rounding-cell interval.','cases':rows,'analytic':{'case':'endpoint-free-wire','radius':.2,'segments':8,'edge_length':edge_length,'box_dimensions':dimensions,'formula':'abc-L*r^2*(1-N*sin(pi/(2*N))/2)','expected_volume':float(expected),'native_binary64_volume_interval':[float(v)for v in native_interval]},'negative_controls':negatives,'scope':'Six retained closed fillet bodies, two corrected oblique/turned production models. Open neighbor faces excluded explicitly; no new geometry, app run or full P09 claim.'}
 Path(sys.argv[2]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
