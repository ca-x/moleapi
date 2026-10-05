#!/usr/bin/env python3
"""Verify worker dispatch in an actual server or desktop release executable."""
import argparse
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", nargs="?")
parser.add_argument("--container")
parser.add_argument("--diagnostics", action="store_true", help="Capture bounded process/architecture evidence on timeout")
args = parser.parse_args()
if args.container:
    if os.environ.get("GITHUB_ACTIONS") != "true":
        parser.error("Container verification runs only in GitHub Actions")
    command = ["docker", "exec", "--interactive", args.container, "env", "-i",
               "/usr/local/bin/moleapi-server", "--moleapi-script-worker"]
    environment = None
    label = "Docker container"
else:
    if not args.binary:
        parser.error("Provide the server or desktop executable")
    binary = Path(args.binary).resolve()
    if os.name == "nt":
        binary = binary.with_suffix(".exe")
    if sys.platform == "darwin" and binary.name == "moleapi":
        bundled = list(binary.parent.glob("bundle/macos/*.app/Contents/MacOS/moleapi"))
        if len(bundled) == 1:
            binary = bundled[0]
    if not binary.is_file():
        raise SystemExit(f"Missing worker executable: {binary}")
    command = [str(binary), "--moleapi-script-worker"]
    environment = {}
    label = binary.name
request = {
    "id": "ci-worker", "name": "Worker verification", "method": "GET",
    "url": "https://example.com/", "description": "", "query": [], "headers": [],
    "body_kind": "none", "body": "", "timeout_ms": 1000,
    "follow_redirects": True, "verify_tls": True,
    "auth": {"kind": "none", "token": "", "username": "", "password": ""},
    "assertions": [], "examples": [],
}
payload = {
    "scripts": ["console.log('platform-worker-ok'); pm.test('worker math',()=>pm.expect(2+2).to.equal(4));"],
    "request": request, "response": None, "private_values": [],
    "scopes": {scope: {} for scope in ["project", "collection", "environment", "data", "temporary"]},
}
started = time.monotonic()
process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, env=environment)
try:
    stdout, stderr = process.communicate(json.dumps(payload).encode(), timeout=10)
except subprocess.TimeoutExpired:
    # Diagnostics never turn a timeout into success or extend the worker acceptance limit.
    if args.diagnostics or os.environ.get("GITHUB_ACTIONS") == "true":
        print(f"Worker timeout evidence: host={platform.machine()} platform={sys.platform} "
              f"pid={process.pid} elapsed={time.monotonic()-started:.3f}s", flush=True)
        if sys.platform == "darwin" and not args.container:
            def observe(arguments, limit=3):
                try:
                    observation = subprocess.run(arguments, stdout=subprocess.PIPE,
                                                 stderr=subprocess.STDOUT, timeout=limit, check=False)
                    print(observation.stdout.decode(errors="replace")[:32768], flush=True)
                except (OSError, subprocess.TimeoutExpired) as error:
                    print(f"Diagnostic unavailable: {error}", flush=True)
            observe(["/usr/bin/file", str(binary)])
            observe(["/bin/ps", "-p", str(process.pid), "-o", "pid,ppid,stat,etime,time,comm"])
            with tempfile.TemporaryDirectory(prefix="moleapi-worker-sample-") as directory:
                sample = Path(directory) / "sample.txt"
                observe(["/usr/bin/sample", str(process.pid), "1", "10", "-file", str(sample)])
                if sample.exists():
                    print(sample.read_text(errors="replace")[:32768], flush=True)
    process.kill()
    process.communicate(timeout=3)
    if (args.diagnostics or os.environ.get("GITHUB_ACTIONS") == "true") and sys.platform == "darwin" and not args.container:
        # A/B/A distinguishes an empty-environment bootstrap issue from a warmed cache.
        for name, isolated in [("fixed-marker", {"MOLEAPI_WORKER_ISOLATED": "1"}), ("empty-again", {})]:
            probe_started = time.monotonic()
            try:
                probe = subprocess.run(command, input=json.dumps(payload).encode(),
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=isolated, timeout=10, check=False)
                try:
                    probe_status = json.loads(probe.stdout).get("status", "missing")
                except (ValueError, AttributeError):
                    probe_status = "invalid-json"
                print(f"Worker environment probe: case={name} exit={probe.returncode} "
                    f"status={probe_status} elapsed={time.monotonic()-probe_started:.3f}s "
                    f"stdout_bytes={len(probe.stdout)} stderr_bytes={len(probe.stderr)}", flush=True)
            except subprocess.TimeoutExpired:
                print(f"Worker environment probe: case={name} timeout=10s", flush=True)
    raise
if process.returncode:
    raise subprocess.CalledProcessError(process.returncode, command, stdout, stderr)
reply = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
result = json.loads(reply.stdout)
assert result["status"] == "success", "Worker returned failure"
assert result["output"]["logs"] == [{"level": "log", "message": "platform-worker-ok"}]
assert len(result["output"]["tests"]) == 1 and result["output"]["tests"][0]["passed"]
assert reply.stderr == b"", "Worker unexpectedly emitted stderr"
print(f"Headless worker dispatch verified: {label} ({sys.platform})")
