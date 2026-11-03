# beui-text-editor-core works on `Sequence<u8>` directly

beui-text-editor-core depends on `be-lib-sequence` and works on `Sequence<u8>`:
cursors and selections are positions that survive other people's edits, and
change logs come from the splices `apply` reports. `Sequence` lives in its own
crate (serde only), and `be-block-model` wraps it as the `Text` field. beui's
inputs, which have no `be-block-model`, use a small undo stack over
`Sequence::inverse` beside the sequence; the text block uses `be-block-model`'s
`Step`; both follow the same rules.

## Why

- The editor and beui's text inputs can use the sequence without the block
  stack, so the editor still works outside this project.

## Rejected

- Keeping the editor generic and having the text block translate its anchors.
