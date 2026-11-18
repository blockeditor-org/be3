# Type modules keep `similar` for line merges

be-block-model's sequence merge (`be_block_commit::merge_lines`) diffs lines with the `similar`
crate, which every type module links. It stays: the user decided in November 2026 not to
replace it.

## Why

Replacing it means writing the line diff `merge_lines` needs (Myers over slices) in
be-block-model or be-lib-sequence: a design change to the merge, not a split, worth it only if
it measurably shrank the modules. `similar` was about 190 KB of a dev checklist module, and a
release build drops most of that.

## Rejected

- A line diff of our own in place of `similar`, to shrink type modules.
