# Text on the shared sequence

Depends on: none. The oversized edit uses the operation that adds a part by
its hash (`put-part`, guides/type_modules.md, "Parts named by hash").

Text and lists share one ordered sequence: `Sequence<T>` in `be-lib-sequence`,
wrapped by `be-block-model` as the `Text` field and as `Items` for lists. The
session code is `be-sdk-type-guest/src/replica.rs`. What is left is below.

## An oversized edit is an operation like any other

Today an edit over `LARGEST_RELAYED_OPERATION` (1 MiB, a large paste) is not
relayed. A follower stashes a commit holding its whole content and tells the
owner `Moved { head }`; the owner reloads that head and `replacement` replays
all its own unsealed operations onto it (BE3-184). Those operations name
positions the reloaded content does not have, since loading gives fresh
positions, and some may already be in the follower's commit, so text can land
on the wrong bytes or twice.

Send an oversized operation like any other instead: its payload is uploaded as
an object and the operation is relayed by its hash (the operation that adds a
part by its hash, guides/type_modules.md, "Parts named by hash"), so the
owner sequences it and every replica applies it at the positions it names.
A payload part is not served yet: `LiveEdit::apply` is synchronous, so a
replica must read the payload by its ref (`read-part`) before it applies the
operation, holding back what is sequenced after it until the read answers.
Since no commit holds a payload part, its hold lasts the whole upload grace. The oversized path through
`Moved`, `Then::Absorb` and `replacement` goes; `replace_content` stays for
real replacements. Done: a test where a follower pastes over 1 MiB while the
owner has unsealed edits ends with every replica holding the same, intended
bytes, and undo removes the paste alone.

## Not planned here

- Optimizing for one large edit, such as indenting every line of a large file
  (an insert at every line start). It is a bad case by nature and rare; it
  gets faster as the sequence does in general, and it should not shape the
  sequence's design.
- Inline text fields and splices for other editors:
  plans/deferred/text.md.
