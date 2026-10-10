#!/usr/bin/env python3
"""Operative frozen transactional eval/exec wrapper; synthetic extension tests native module registration only."""
import os,sys,json,types
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();vp=types.SimpleNamespace(scene=s,history=History(s),notify_scene_changed=lambda:None)
m=types.ModuleType("native_probe");m.add=lambda a,b:a+b;sys.modules["native_probe"]=m
scope={};code=(Path(__file__).resolve().parents[1]/"tools/ai-interpreter-proof/language.py").read_text()
first=run_transactional(vp,code,scope);expression=run_transactional(vp,"counter.twice()",scope);none=run_transactional(vp,"None",scope);advance=run_transactional(vp,"assert _ == 570\ncounter.value += 1\nassert counter.twice()==572",scope);syntax=run_transactional(vp,"def broken(",scope);error=run_transactional(vp,"raise ValueError('literal-failure')",scope)
print(json.dumps({"captured_stdout":first["stdout"]+expression["stdout"]+advance["stdout"],"persistent_value":repr(scope["counter"].twice()),"runtime_exception":error["error"] is not None,"syntax_refused":syntax["error"] is not None,"traceback":error["stderr"],"unchanged_history":len(vp.history.undo_stack)==0,"changed_flags":[first["changed"],expression["changed"],advance["changed"]]},indent=2))
