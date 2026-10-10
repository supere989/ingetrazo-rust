"""Minimal provenance for reproducible acceptance records."""
import subprocess,sys

def context():
    def revision(directory):
        proc=subprocess.run(["git","-C",directory,"rev-parse","HEAD"],capture_output=True,text=True,check=True)
        return proc.stdout.strip()
    return {"rust_commit":revision("."),"original_python_commit":revision("ingetrazo"),"checker_python":sys.version.split()[0],"boundary":"Evidence from this checkout; retained baseline failures are historical, not current verdicts."}
