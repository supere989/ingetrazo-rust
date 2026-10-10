#!/usr/bin/env python3
"""Exact source object lifetime, including actual retained old leak."""
import json,sys
from pathlib import Path
a=json.loads(Path(sys.argv[1]).read_text())["states"];b=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
assert len(a)==len(b)==len(old)==3
assert a==b and [r["old_alive"]for r in a]==[True,True,False]
assert old[:2]==a[:2] and old[-1]["old_alive"] and old[-1]!=a[-1]
print(json.dumps({"passed":True,"states":3,"old_leak_rejected":True,"full_memory_claim":False}))
