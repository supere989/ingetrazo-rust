"""Fresh original-source unit comparison; native executable extracts production functions."""
import sys,struct,subprocess,json,random
sys.path.insert(0,sys.argv[1])
from core.units import format_length
rng=random.Random(91831)
values=[0.,-0.,.3048,.6096,1.,1e10,1e20,1e30,3e38]+[struct.unpack('f',struct.pack('f',rng.uniform(-1e4,1e4)))[0] for _ in range(100)]
rows=[(v,u,n) for v in values for u in ['m','cm','mm','in','ft','ft-in','in-frac','ft-in-frac'] for n in [0,1,2,4,6]]
inputs=''.join(f'{struct.unpack("Q",struct.pack("d",v))[0]} {u} {n}\n' for v,u,n in rows)
out=subprocess.check_output([sys.argv[2]],input=inputs,text=True).splitlines()
assert len(out)==len(rows)
fail=[dict(meters=v,unit=u,decimals=n,original=format_length(v,u,n),native=s) for (v,u,n),s in zip(rows,out) if format_length(v,u,n)!=s]
print(json.dumps({'cases':len(rows),'failures':fail},indent=2))
assert not fail
