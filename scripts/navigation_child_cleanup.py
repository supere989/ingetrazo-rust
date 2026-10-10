"""Bounded retirement of only this harness's Popen children."""
import subprocess,time

def terminate_owned(child,timeout=3):
 start=time.monotonic();result=dict(pid=child.pid,terminate_timeout=False,kill_timeout=False,returncode=child.poll())
 if result['returncode'] is None:
  child.terminate()
  try:result['returncode']=child.wait(timeout=timeout)
  except subprocess.TimeoutExpired:
   result['terminate_timeout']=True;child.kill()
   try:result['returncode']=child.wait(timeout=timeout)
   except subprocess.TimeoutExpired:result['kill_timeout']=True
 result['elapsed_seconds']=time.monotonic()-start
 return result
