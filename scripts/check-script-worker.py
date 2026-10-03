#!/usr/bin/env python3
"""Verify worker dispatch in an actual server or desktop release executable."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", nargs="?")
parser.add_argument("--container")
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
reply = subprocess.run(
    command, input=json.dumps(payload).encode(),
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment, timeout=10, check=True,
)
result = json.loads(reply.stdout)
assert result["status"] == "success", "Worker returned failure"
assert result["output"]["logs"] == [{"level": "log", "message": "platform-worker-ok"}]
assert len(result["output"]["tests"]) == 1 and result["output"]["tests"][0]["passed"]
assert reply.stderr == b"", "Worker unexpectedly emitted stderr"
print(f"Headless worker dispatch verified: {label} ({sys.platform})")
