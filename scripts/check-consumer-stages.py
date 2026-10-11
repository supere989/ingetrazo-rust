#!/usr/bin/env python3
"""Small process faults only; no project build, original capture or consumer replay."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from consumer_stages import Stages

with tempfile.TemporaryDirectory(prefix="consumer-stage-faults-") as temporary:
    out = Path(temporary)
    (out/"result.json").write_text('{"result":"pass","old":true}')
    (out/"old-native.json").write_text('{"old":true}')
    stages = Stages(out)
    assert not (out/"result.json").exists()
    def child(code, timeout=1):
        return subprocess.run([sys.executable,"-c",code], check=True, timeout=timeout,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stages.stage("failed-producer", [], lambda: child("raise SystemExit(7)"))
    stages.stage("dependent-reader", ["failed-producer"], lambda: (out/"reader-ran").write_text("wrong"))
    stages.stage("dependent-checker", ["dependent-reader"], lambda: json.loads((out/"old-native.json").read_text()))
    stages.stage("unrelated-producer", [], lambda: child("print('independent')"))
    stages.stage("unrelated-checker", ["unrelated-producer"], lambda: child("print('checked')"))
    stages.stage("timeout", [], lambda: child("import time; time.sleep(1)", .05))
    stages.stage("timeout-dependent", ["timeout"], lambda: (out/"timeout-reader-ran").write_text("wrong"))
    stages.stage("failed-archive", [], lambda: (_ for _ in ()).throw(ValueError("bad digest")))
    stages.stage("archive-dependent-producer", ["failed-archive"], lambda: child("raise SystemExit('should not execute')"))
    with contextlib.redirect_stdout(io.StringIO()):
        status = stages.finish(lambda: (out/"summary-ran").write_text("wrong"))
    report=json.loads((out/"result.json").read_text())
    assert status == 1 and report["result"] == "fail" and "old" not in report
    assert report["failed"] == ["failed-producer","timeout","failed-archive"]
    assert stages.records["failed-producer"]["returncode"] == 7
    assert stages.records["timeout"]["timeout_seconds"] == .05
    assert stages.records["unrelated-checker"]["status"] == "pass"
    assert not any((out/p).exists() for p in ["reader-ran","timeout-reader-ran","summary-ran"])
    success=Stages(out/"success")
    success.stage("producer",[],lambda: child("print('good')"))
    success.stage("checker",["producer"],lambda: child("print('good')"))
    expected={"result":"pass","kept_success_schema":17}
    with contextlib.redirect_stdout(io.StringIO()):assert success.finish(lambda:expected) == 0
    assert json.loads((out/"success/result.json").read_text()) == expected
    print(json.dumps({"process_failure_returncode":7,"dependents_skipped":4,"independent_stages_passed":2,
                      "timeout_failed":True,"archive_failure_blocks_producer":True,
                      "old_success_retired":True,"stale_native_not_read":True,"failure_summary_not_called":True,
                      "success_schema_preserved":True,"observed_status":{k:r["status"] for k,r in stages.records.items()},"result":"pass"}))
