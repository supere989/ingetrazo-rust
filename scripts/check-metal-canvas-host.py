#!/usr/bin/env python3
"""Bounded capture of the actual application probe's own Canvas window."""
import argparse, hashlib, json, pathlib, subprocess, time
p = argparse.ArgumentParser()
p.add_argument("binary", type=pathlib.Path)
p.add_argument("output", type=pathlib.Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=False)
root = a.output / "host"
report = {"passed": False, "pixel_parity_claim": False, "physical_mac_claim": False,
          "binary_sha256": hashlib.sha256(a.binary.read_bytes()).hexdigest()}
with (a.output / "host.log").open("wb") as log:
    proc = subprocess.Popen([str(a.binary.resolve()), "--choice=canvas", "--output=" + str(root.resolve())], stdout=log, stderr=subprocess.STDOUT)
    try:
        started = time.monotonic()
        identity = root / "own-window.json"
        while not identity.exists():
            if proc.poll() is not None:
                raise RuntimeError("Host exited before Canvas presentation")
            if time.monotonic() - started > 10:
                raise RuntimeError("Canvas presentation deadline")
            time.sleep(.025)
        snapshot = json.loads(identity.read_text())
        window = snapshot["window_id"]
        if type(window) is not int or not 0 < window < 2**32 or snapshot["displayed"] <= 0:
            raise RuntimeError("Invalid own-window presentation identity")
        raw = snapshot.get("raw_uploads")
        if not isinstance(raw, list) or len(raw) != 5 or raw[0:2] != [0, 0] or raw[2] <= 0 or raw[2] != raw[4] or snapshot.get("warm_frames", 0) < 3:
            raise RuntimeError("Missing acknowledged Canvas texture and stable warm uploads")
        report["snapshot"] = snapshot
        image = a.output / "canvas.png"
        result = subprocess.run(["/usr/sbin/screencapture", "-x", "-o", "-l" + str(window), str(image.resolve())], capture_output=True, timeout=2)
        (a.output / "capture.stderr").write_bytes(result.stderr)
        result.check_returncode()
        data = image.read_bytes()
        if not data.startswith(b"\x89PNG\r\n\x1a\n"):
            raise RuntimeError("Capture did not produce PNG")
        report["image_sha256"] = hashlib.sha256(data).hexdigest()
    except Exception as e:
        report["error"] = repr(e)
    finally:
        if root.is_dir():
            (root / "capture-finished").write_text("capture completed or refused\n")
        try:
            report["exit_code"] = proc.wait(timeout=18)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
            report["error"] = "Host exceeded shutdown deadline"
        result_file = root / "result.json"
        if result_file.exists():
            report["host"] = json.loads(result_file.read_text())
        report["passed"] = ("error" not in report and report.get("exit_code") == 0 and report.get("host", {}).get("passed") is True)
        (a.output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
if not report["passed"]:
    raise SystemExit(1)
