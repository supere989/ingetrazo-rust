"""Bounded teardown of an already-owned disposable helper; no process discovery."""
import json,subprocess
def stop_owned_helper(process,report,timeout=5):
 state={"pid":process.pid,"terminate_timeout":False,"kill_sent":False}
 if process.poll() is None:
  process.terminate()
  try:process.wait(timeout=timeout)
  except subprocess.TimeoutExpired:
   state["terminate_timeout"]=True
   if process.poll() is None:
    process.kill();state["kill_sent"]=True
   process.wait(timeout=timeout)
 state["returncode"]=process.returncode
 report.write_text(json.dumps(state,indent=2)+"\n")
 return state
