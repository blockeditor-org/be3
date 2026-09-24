# rustc's check pass over every first-party target and configuration - host,
# plugins and games - on BuildBuddy, failing with the compiler's errors.
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
