"""Report observed candidate behavior without equating QA execution with interop."""
import json
import sys
from pathlib import Path

log = Path(sys.argv[1]).read_text()
exit_code = int(Path(sys.argv[2]).read_text().strip())
if exit_code:
    verdict = "candidate_failed"
elif "candidate_value_utf8=echo:moleapi" in log:
    verdict = "candidate_echo_compatible"
else:
    verdict = "candidate_echo_incompatible"
print(json.dumps({"verdict": verdict, "candidate_exit": exit_code,
                  "response_status": next((line.split("=", 1)[1] for line in log.splitlines()
                                           if line.startswith("response_status=")), None)}, sort_keys=True))
