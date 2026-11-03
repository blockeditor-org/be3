# Visual tests compare paint snapshots, not screenshots

A test's picture of a frame is a paint snapshot (`snapshots/NAME.paint`): beui-paint-format's drawing commands as CBOR, deflated, with each glyph carrying its own image keyed by its contents, compared exactly. Browser pages are snapshotted as their document and resources (`NAME.browser.paint`) and rendered only for review. See guides/testing_a_gui.md, 4.

## Why

- A painting is a few kilobytes rather than the hundreds a screenshot costs, and frames of a recording share one table of textures.
- It is compared exactly, so nothing about it is flaky.
- Glyph images are keyed by what is in them rather than by where they landed in an atlas, so text an earlier frame drew cannot change a later frame's snapshot.
- Accepting compares the CBOR before deflating it, so a painting passes or fails on what it holds.

## Rejected

- PNG screenshots: hundreds of kilobytes each, and not compared exactly.
- Storing the glyph atlases: where a glyph lands depends on everything drawn before it.
