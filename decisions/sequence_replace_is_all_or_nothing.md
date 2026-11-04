# A sequence replace applies only if all it replaces is still visible

`SeqOp::Replace` applies only if every span is still visible; the editor asks
for it with `DocumentEdit::replace_atomically`, which toggling a markdown
checkbox uses. Typing over a selection stays a delete and an insert.

## Why

- Two people checking the same markdown box produce `[x]`, not `[xx]`.
- Typing over a selection as a delete and an insert keeps what you typed when
  a collaborator edits inside the selection at the same time.
