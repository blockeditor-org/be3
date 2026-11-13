# Search: set aside

Depends on: none.

Set aside from plans/search.md until the user picks them up. The limits below
are accepted for now.

- **Fuzzy matching on names**, which needs a way to find a misspelt name's
  terms without reading a segment's whole dictionary (the blocked dictionary
  answers exact terms and prefixes only). Set aside by the owner.
- **PDF text** answering `search-text`. Set aside by the owner. Not by linking
  pdfium into the pdf type module, which the owner ruled out.
- **No host test for a second device.** be-host-tests start one host with its own
  dev server, so catching up what another device wrote, and a host restart, are
  only tested in be-host-stack: a harness for a second device and a host restart. Set aside: be-host-stack's tests cover both.

## Limits accepted

- **Leaving the tree is masked on the next connection** (`changed-since`, built),
  not at once: a block moved to a private root, trashed or emptied keeps its text
  in the old root's index until a device that may edit the root next catches up,
  and only the catch-up masks it (no mask at the moment of the move). Until then
  every viewer of that root can decrypt it with a modified client; the UI hides it
  (`show` filters by access and by what the search is within). Departures are kept a
  day on the server: a device that was away longer misses every block that left in
  between, moved out, trashed or collected (it is only hidden by `show`).
- **A viewer's device does not catch up.** Only an editor of the root writes its
  index, so a viewer's searches show the text the last editor catch-up indexed.
  plans/deferred/search_personal_index.md would cover it.
- **Edited outside a session.** The indexer seals commits straight into the
  outbox rather than through the index block's replica (`put-part` plus a
  `SetIndex` operation): two devices online at once diverge and merge by parts
  instead of being sequenced. `LiveEdit` has no hook for a replica to put a
  part of its own, which a session needs.
- **Late text in a huge document is not searchable.** A document's text is
  indexed up to 1 MiB (`MAX_TEXT`); a word past that finds nothing, and nothing
  in the interface says why. It is capped after the body is loaded, so it does
  not bound memory, and the number of documents per block is not capped.
- **A trashed index is not restored.** From Detached it can only return to
  where it was, which the server refuses for an index; a new one is made.
