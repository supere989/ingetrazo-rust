import json,sys
from pathlib import Path
a=json.loads(Path(sys.argv[1]).read_text());b=json.loads(Path(sys.argv[2]).read_text());assert [x["text"]for x in a["states"]]==["3x","/3","2x"]==[x["text"]for x in b["states"]]
assert a["states"][0]==b["states"][0],"First unmixed array must match literally"
diff=[]
for label,x,y in [(str(i),x,y)for i,(x,y)in enumerate(zip(a["states"],b["states"]))]+[("undo",a["undo"],b["undo"]),("redo",a["redo"],b["redo"])]:
 for k in x:
  if x[k]!=y[k]:diff.append({"state":label,"field":k,"source":x[k],"native":y[k]})
print(json.dumps({"passed":not diff,"differences":diff,"boundary":"Original retyping/Undo leaves authored holed face boundary wires; Rust restores complete pre-copy mesh."},indent=2))
if "--retain-open"in sys.argv:
 assert diff and {d["field"]for d in diff}=={"edges","edge_segments"}
 assert [x["edges"]for x in a["states"]]==[40,56,54] and a["undo"]["edges"]==50 and a["redo"]["edges"]==54
 assert [x["edges"]for x in b["states"]]==[40,40,30] and b["undo"]["edges"]==10 and b["redo"]["edges"]==30
else:assert not diff,"Literal original geometry parity remains open"
