# Backlog: set aside

Depends on: none.

Items set aside until the user picks them up.

## Block size

- **A text block's reported size grows fast.** long1.md, about 30 KB of Markdown, showed as
  "Text · 31 KB" in the files list after import and "Text · 154 KB" after a handful of short
  edits and a few restarts; tiny.md, with about 21 KB pasted in, showed 22 KB. Likely revision
  history counted in the size, or kept more than it needs to be.

## beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one. No reproduction yet, and a headless machine has no real
  wheel: it needs the backend (winit, DRM and libinput, or the web) and an
  input sequence to replay before it can be fixed.

- **be-host's overflow check flags a child slid or scrolled by its parent.** be-host clips
  each child view to the clip window its parent reports (`compositor/editors.rs`), and cannot
  see a translation or scroll inside the parent app's document, so a host painting of a back
  swipe or a half-scrolled child fails as "cut off by a clip nothing scrolls". Let the parent
  report that the child's window moves with its content, and exempt it as a pannable canvas is.

## Loading part of a document

Documents save as parts chosen by their structure but still load whole
(guides/blocks/content_types.md, "The stored form"). Let the model load part
of one: an attribute such as `#[model(lazy)]` on a `List` whose parts' objects
load when first read, through the `read-part` path a `Loads::FirstPart` type
already uses. Saving hands an unloaded part back as `Reused`, which keeps its
node's own references (`Part::Node { references }`,
`be-block-model/src/parts.rs`), so such a document drops only the unloaded
parts' references from `references_in`. Done: a document with a lazy list opens
without reading that list's parts, and saving it reuses them with their
references. Until this lands, documents merge as whole bytes (`merge3`), not by parts
(guides/type_modules.md, "Merging by parts").

Set aside by the owner.

## Reconnecting on Android

- **Why a reconnect hits "the session has a newer owner"** is still not known:
  a test's drop and reconnect did not meet it. The error no longer outlives its
  cause: it clears once the device is connected with nothing waiting (`report`
  in be-host-stack's worker.rs). Set aside: not reproduced, and the error now clears.
