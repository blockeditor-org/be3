# clippy over the workspace through buck2, which is what //:verify's lint pass
# runs.
#
# buck/dev/workspace.bxl builds every first-party Rust target's [clippy.json] -
# rustc's JSON diagnostics - in every configuration the workspace builds it in,
# on BuildBuddy's workers. This reads them. A file compiled in more than one
# configuration or target is diagnosed more than once, so each finding is
# reported once. The toolchain denies warnings, so any finding fails the run.
#
# With --fix it first applies the suggestions clippy marks machine-applicable,
# which is what `cargo clippy --fix` applies: every edit a diagnostic suggests
# goes in together or not at all, one that overlaps an edit already taken is
# left for the next run, and then clippy runs again over the result, so what is
# reported is what the fixes did not reach.

import json
import os
import subprocess
import sys


def diagnostics(buck):
    listing = subprocess.run(
        [buck, "bxl", "//buck/dev/workspace.bxl:subtarget", "--", "--subtarget", "clippy.json"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout
    found = []
    for line in listing.splitlines():
        if "\t" not in line:
            continue
        _, path = line.split("\t", 1)
        with open(path) as file:
            for entry in file:
                entry = entry.strip()
                if entry:
                    found.append(json.loads(entry))
    return found


def first_party(diagnostic):
    return any(span["file_name"].startswith("crates/") for span in diagnostic.get("spans", []))


def suggestions(diagnostic):
    edits = []
    pending = [diagnostic]
    while pending:
        current = pending.pop()
        for span in current.get("spans", []):
            if span.get("suggested_replacement") is None:
                continue
            if span.get("suggestion_applicability") != "MachineApplicable":
                continue
            edits.append((span["file_name"], span["byte_start"], span["byte_end"], span["suggested_replacement"]))
        pending.extend(current.get("children", []))
    return edits


def fix(found):
    chosen = {}
    applied = 0
    seen = set()
    for diagnostic in found:
        edits = sorted(set(suggestions(diagnostic)))
        if not edits or not all(edit[0].startswith("crates/") for edit in edits):
            continue
        key = tuple(edits)
        if key in seen:
            continue
        seen.add(key)
        overlaps = any(
            start < other_end and other_start < end
            for path, start, end, _ in edits
            for other_start, other_end, _ in chosen.get(path, [])
        )
        if overlaps:
            continue
        for path, start, end, replacement in edits:
            chosen.setdefault(path, []).append((start, end, replacement))
        applied += 1
    for path, edits in chosen.items():
        with open(path, "rb") as file:
            source = file.read()
        for start, end, replacement in sorted(edits, reverse=True):
            source = source[:start] + replacement.encode() + source[end:]
        with open(path, "wb") as file:
            file.write(source)
    return applied


def report(found):
    rendered = []
    for diagnostic in found:
        if diagnostic.get("level") not in ("error", "warning") or not first_party(diagnostic):
            continue
        text = diagnostic.get("rendered") or diagnostic["message"]
        if text not in rendered:
            rendered.append(text)
    for text in rendered:
        sys.stdout.write(text)
    return len(rendered)


def run(buck, fixing):
    found = diagnostics(buck)
    if fixing and found:
        applied = fix(found)
        if applied:
            print("Applied the fixes of {} clippy findings.".format(applied))
            found = diagnostics(buck)
    count = report(found)
    if count:
        print("clippy: {} findings.".format(count))
    return count == 0
