#!/usr/bin/env python3
"""Fetch the pinned build-time generator; the product never downloads engines."""
import hashlib
import json
from pathlib import Path
import urllib.request
root = Path(__file__).resolve().parents[1] / "vendor/openapi-generator"
manifest = json.loads((root / "manifest.json").read_text())
output = root / "engine.jar"
if output.is_file() and hashlib.sha256(output.read_bytes()).hexdigest() == manifest["sha256"]:
    print("Pinned OpenAPI generator already verified")
else:
    with urllib.request.urlopen(manifest["url"], timeout=60) as response:
        data = response.read(48 * 1024 * 1024 + 1)
    if len(data) > 48 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != manifest["sha256"]:
        raise SystemExit("Generator asset SHA256/size verification failed")
    temporary = output.with_suffix(".download")
    temporary.write_bytes(data)
    temporary.replace(output)
    print("Pinned OpenAPI generator downloaded and verified")
