#!/usr/bin/env python3
"""Strict pinned Qt float32 Move key controls; not general coordinate parity."""
import json,sys
from pathlib import Path
original,native=[json.loads(Path(p).read_text()) for p in sys.argv[1:]]
assert len(original)==len(native)==16,"required key inventory missing"
assert original==native,"source Qt key mismatch"
assert original[7]!=original[8] and original[9]!=original[10],"large distinct positions collapsed"
assert original[13:]==[None,None,None],"nonfinite/out-of-f32 controls missing"
print(json.dumps({"cases":16,"exact":True,"scope":"Pinned Qt float32 Move key controls"}))
