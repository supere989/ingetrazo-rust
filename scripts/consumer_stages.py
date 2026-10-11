"""Sequential consumer DAG: collect failures without reading failed producer outputs."""
import json
from pathlib import Path
import subprocess
import time

class Stages:
    def __init__(self, output):
        self.output = Path(output)
        self.output.mkdir(parents=True, exist_ok=True)
        self.records = {}
        # A crashed/reused run must never leave an earlier successful result.
        for name in ("result.json", "stages.json"):
            (self.output / name).unlink(missing_ok=True)

    def stage(self, name, requires, action):
        if name in self.records:
            raise ValueError("Duplicate stage: " + name)
        requires = list(requires)
        blocked = [p for p in requires if self.records.get(p, {}).get("status") != "pass"]
        row = {"requires": requires}
        self.records[name] = row
        if blocked:
            row.update(status="skip", blocked_by=blocked)
            return None
        started = time.monotonic()
        try:
            value = action()
        except Exception as error:
            row.update(status="fail", error=type(error).__name__ + ": " + str(error))
            if isinstance(error, subprocess.CalledProcessError):
                row["returncode"] = error.returncode
            if isinstance(error, subprocess.TimeoutExpired):
                row["timeout_seconds"] = error.timeout
            value = None
        else:
            row["status"] = "pass"
        row["seconds"] = round(time.monotonic() - started, 6)
        return value

    def finish(self, summarize):
        if all(r["status"] == "pass" for r in self.records.values()):
            result = self.stage("summary", [], summarize)
        else:
            result = None
        if result is None:
            result = {"result": "fail", "failed": [k for k, r in self.records.items() if r["status"] == "fail"],
                      "skipped": [k for k, r in self.records.items() if r["status"] == "skip"]}
        (self.output / "stages.json").write_text(json.dumps(self.records, indent=2) + "\n")
        (self.output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result))
        return 0 if result.get("result") == "pass" else 1
