#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
def check(original,native):
 a=json.loads(Path(original).read_text());b=json.loads(Path(native).read_text())
 assert set(a)==set(b)=={"requests","detect","vision","code","compact","thoughts"}
 assert {k:len(v)for k,v in a.items()}=={"requests":14,"detect":8,"vision":6,"code":5,"compact":3,"thoughts":5},"Missing source cases"
 assert a==b,"Provider wire/schema mismatch"
 for section in a:
  bad=copy.deepcopy(b);bad[section][0]="MUTATED";assert bad!=a
 return {"passed":True,"cases":41,"negative_sections_rejected":list(a),"scope":"Offline pinned source provider wire; no network/executor/MCP behavior"}
if __name__=="__main__":print(json.dumps(check(*sys.argv[1:3]),indent=2))
