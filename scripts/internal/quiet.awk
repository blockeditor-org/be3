# Cuts buck2's console output down to what explains a failure: its progress
# and cache lines go, and so does the output of every test target that passed
# and every test inside a failing target that passed. What is left is the
# compiler's errors and the failing tests' output.
#
# Usage: awk -f scripts/internal/quiet.awk < log

function stamp_of(line) {
    if (match(line, /^\[[^]]*\]/)) {
        return substr(line, 1, RLENGTH)
    }
    return ""
}

{
    stamp = stamp_of($0)
    text = substr($0, length(stamp) + 1)
    if (stamp != "") {
        sub(/^ /, "", text)
    }

    if (passed != "" && stamp == passed && text !~ /^[✓✗] /) {
        next
    }
    passed = ""

    if (text ~ /^✓ Pass: /) {
        passed = stamp
        next
    }
    if (text ~ /^Skipped [0-9]+ incompatible targets:/) {
        skipping = 1
        next
    }
    if (skipping && text ~ /^  /) {
        next
    }
    skipping = 0

    if (text ~ /^(Build ID|File changed|Directory changed|RE Session|Cache hits|Commands|Network|Resource usage|IO):/) next
    if (text ~ /^Waiting on /) next
    if (text ~ /^[0-9]+ additional file change events/) next
    if (text ~ /^(BUILD|BXL) SUCCEEDED/) next
    if (text ~ /^test .* \.\.\. ok$/) next

    print text
}
