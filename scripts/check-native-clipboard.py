#!/usr/bin/env python3
"""Native X11 clipboard acceptance on a disposable server, never the desktop."""
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
from acceptance_context import context


def main():
    report = {
        "provenance": context(),
        "passed": False,
        "transport": "disposable X11 native clipboard ownership",
        "scope": "Independent owners transfer geometry; unrelated text, malformed JSON and invalid topology clear cached geometry atomically; a replacement malformed request retires the previous valid preview. Wayland, Windows/macOS and clipboard-manager persistence are unverified.",
    }
    try:
        with tempfile.TemporaryDirectory(prefix="ingetrazo-clipboard-x11-") as temporary:
            # Startup can emit more diagnostics than an unread pipe can hold.
            # A file preserves them without blocking display allocation.
            with tempfile.TemporaryFile(mode="w+t") as diagnostics:
                server = subprocess.Popen(
                    ["Xvfb", "-displayfd", "1", "-screen", "0", "800x600x24", "-nolisten", "tcp"],
                    stdout=subprocess.PIPE, stderr=diagnostics, text=True,
                )
                try:
                    if not select.select([server.stdout], [], [], 10)[0]:
                        raise RuntimeError("Xvfb did not allocate a display within 10 seconds")
                    display = server.stdout.readline().strip()
                    if not display.isdigit():
                        raise RuntimeError(f"Xvfb returned no valid display: {display!r}")
                    report["display"] = ":" + display
                    env = os.environ.copy()
                    env["DISPLAY"] = report["display"]
                    env.pop("WAYLAND_DISPLAY", None)
                    env["XDG_RUNTIME_DIR"] = temporary
                    command = ["cargo", "test", "--locked", "-p", "ingetrazo-gui", "--test", "clipboard_transport_acceptance", "--", "--ignored", "--nocapture"]
                    report["command"] = command
                    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=180)
                    report.update(passed=result.returncode == 0, output=result.stdout + result.stderr, returncode=result.returncode)
                finally:
                    if server.poll() is None:
                        server.terminate()
                        try:
                            server.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            server.kill()
                            server.wait(timeout=5)
                    server.stdout.close()
                    diagnostics.seek(0)
                    report["xvfb_diagnostics"] = diagnostics.read()
                    report["xvfb_returncode"] = server.returncode
    except Exception as error:
        report["passed"] = False
        report["error"] = f"{type(error).__name__}: {error}"
    finally:
        Path("docs/audits/native-clipboard-semantic-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
