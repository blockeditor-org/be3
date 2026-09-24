# Fast compile feedback: rustc's check pass - type checking and borrow checking
# with no code generated - over every first-party target, tests included, in
# every configuration the workspace builds it in: the host's, and the plugins'
# and games' wasm. It runs on BuildBuddy's workers, and fails with the
# compiler's errors on the first target that has any.
#
# Usage:
#   ./scripts/buck run //:check

import os
import subprocess
import sys

if len(sys.argv) != 1:
    sys.exit("Usage: ./scripts/buck run //:check")

result = subprocess.run(
    [os.path.abspath("scripts/buck"), "bxl", "//buck/dev/workspace.bxl:subtarget", "--", "--subtarget", "check"],
    stdout=subprocess.DEVNULL,
)
sys.exit(result.returncode)
