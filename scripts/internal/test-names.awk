# For scripts/test: reads "FILE:fn NAME" lines, one for each function in a Rust
# file that holds a test, and prints the directory of the crate of each
# function whose full test name (its module path, then its own name) contains
# the variable name.
{
    colon = index($0, ":")
    file = substr($0, 1, colon - 1)
    function_name = substr($0, colon + 4)
    at = index(file, "/src/")
    if (at > 0) {
        crate = substr(file, 1, at - 1)
        relative = substr(file, at + 5)
    } else if (match(file, /\/(tests|examples|benches)\//)) {
        crate = substr(file, 1, RSTART - 1)
        relative = substr(file, RSTART + RLENGTH)
        sub(/^[^\/]+(\/|$)/, "", relative)
    } else {
        next
    }
    sub(/\.rs$/, "", relative)
    sub(/^bin\/[^\/]+(\/|$)/, "", relative)
    sub(/^bin\/[^\/]+$/, "", relative)
    sub(/(^|\/)mod$/, "", relative)
    if (relative == "lib" || relative == "main") {
        relative = ""
    }
    gsub(/\//, "::", relative)
    full = relative == "" ? function_name : relative "::" function_name
    if (index(full, name) > 0) {
        print crate
    }
}
