# Text and lists save their payload only, never their positions

A text field saves its bytes and a list saves its `ObjectId`s. The position
ids, fragments and tombstones of the shared sequence (`be-lib-sequence`) exist
only in the live session. So live edits address positions, but an offline merge
addresses payloads: text merges with line diff3 (conflicts stay visible), lists
with `merge_order`.

## Why

- Partial reads and chunk deduplication keep working on plain bytes.
- Positions are never saved, so an offline merge cannot match them.
