#!/usr/bin/env python3
"""Owned finite HTTP transport for the prebuilt native DEM Profile consumer."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import socket
import subprocess
import threading
import time
from urllib.parse import parse_qs, urlparse

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("source", type=Path)
args = parser.parse_args()
root = args.source.resolve()
data = json.loads((root / "original-dem.json").read_text())
routes, observations = {}, []
lock = threading.Lock()
active = peak = 0
observation_overflow = False
for case in data["cases"]:
    for spec in [case["provider"], case.get("provider_variant")]:
        if not spec:
            continue
        for index, tile in enumerate(spec.get("tiles", case["tiles"])):
            path = spec["path_template"].format(z=tile["z"], x=tile["x"], y=tile["y"])
            assert path.startswith("/") and path != "/__control" and path not in routes
            file = (root / tile["file"]).resolve()
            file.relative_to(root)
            code = case["response_status"][index] if spec is case["provider"] else 200
            gate = threading.Event()
            if case["mode"] in ["cache", "mapbox"]:
                gate.set()
            routes[path] = (gate, file.read_bytes(), code)


def observe(event):
    global observation_overflow
    with lock:
        if len(observations) >= 4096:
            observation_overflow = True
            return False
        observations.append(event)
        return True


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(5)

    def log_message(self, *args):
        pass

    def do_GET(self):
        global active, peak
        parsed = urlparse(self.path)
        if parsed.path == "/__control":
            query = parse_qs(parsed.query)
            action = query.get("action", [""])[0]
            if action == "release":
                path = query.get("path", [""])[0]
                if path not in routes:
                    self.send_error(400)
                    return
                routes[path][0].set()
            elif action == "release-all":
                prefix = query.get("prefix", [""])[0]
                selected = [gate for path, (gate, _, _) in routes.items() if prefix and path.startswith(prefix)]
                if not selected:
                    self.send_error(400)
                    return
                for gate in selected:
                    gate.set()
            elif action != "observe":
                self.send_error(400)
                return
            with lock:
                body = json.dumps(observations).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if not observe({"path": parsed.path, "event": "requested"}):
            self.send_error(503)
            return
        response = routes.get(parsed.path)
        with lock:
            active += 1
            peak = max(peak, active)
        try:
            if response is None:
                self.send_error(404)
                observe({"path": parsed.path, "event": "responded", "status": 404})
                return
            gate, body, code = response
            if not gate.wait(20):
                self.send_error(504)
                observe({"path": parsed.path, "event": "responded", "status": 504})
                return
            self.send_response(code)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            observe({"path": parsed.path, "event": "responded", "status": code})
        except (BrokenPipeError, ConnectionResetError, socket.timeout):
            observe({"path": parsed.path, "event": "disconnected"})
        finally:
            with lock:
                active -= 1


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
# server_close joins these owned handlers; each socket and gate is finite.
server.daemon_threads = False
thread = threading.Thread(target=server.serve_forever, daemon=True)
provider = root / "runtime-provider.json"
assert not provider.exists(), "Native DEM replay must start in a fresh work folder"
provider.write_text(json.dumps({
    "base_url": f"http://127.0.0.1:{server.server_port}", "deadline_ms": 15000,
    "source_pin": data["source_pin"],
    "driver_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
}, indent=2) + "\n")
record = {"argv": [str(args.binary.resolve()), str(root), "--dem-profile"],
          "returncode": None, "loopback_only": True}
started = time.monotonic()
thread.start()
try:
    # Leave time for owned HTTP teardown before the outer 180-second replay bound.
    result = subprocess.run(record["argv"], timeout=150)
    record["returncode"] = result.returncode
except subprocess.TimeoutExpired:
    record.update(returncode=124, error="Native DEM consumer exceeded 150 seconds")
except Exception as error:
    record.update(returncode=1, error=str(error))
finally:
    for gate, _, _ in routes.values():
        gate.set()
    server.shutdown()
    server.server_close()
    thread.join(timeout=5)
    record.update(seconds=time.monotonic() - started, max_http_active=peak,
                  remaining_active=active, observation_overflow=observation_overflow,
                  owned_handlers_joined=active == 0 and not thread.is_alive())
    if observation_overflow or not record["owned_handlers_joined"]:
        record["returncode"] = record["returncode"] or 1
    (root / "native-transport.json").write_text(json.dumps(observations, indent=2) + "\n")
    (root / "native-transport-receipt.json").write_text(json.dumps(record, indent=2) + "\n")
raise SystemExit(record["returncode"])
