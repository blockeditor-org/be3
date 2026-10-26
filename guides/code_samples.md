# Code samples in the docs

A code sample in a Markdown file under guides/ can come from code a test
runs, so it compiles and passes or the build says so:

````
```rust example=checklist.adding_an_item_puts_it_on_the_list
```
````

`example=CRATE.NAME` names `fn example_NAME` or `mod example_NAME` in the
crate whose Cargo.toml `[package]` name is CRATE, in any of its files (a crate
nested inside its directory is a crate of its own). `./scripts/verify` writes
what is inside its braces into the block, dedented: a function's body, or a
module's items. Edit the code and never the block. The docs fix reads the
crates as the Rust autofixes leave them, so a sample shows code formatted and
without comments, as the crate holds it.

- A test runs the example, so it compiles, runs and is not dead code. Keep an
  example in the test file that runs it.
- What the sample leaves out is the example's parameters and whatever its test
  does around it: `fn example_recording_a_gesture(editor: &mut
  BeuiTest<ChecklistApp>)` shows the gesture and not the setup that built the
  editor.
- A whole program, or anything that needs its `use` lines, imports and items,
  is a module: `mod example_counter_app` in
  crates/beui-demo/src/tests/a_counter_app_counts_its_clicks.rs is the app,
  and the test beside it starts it, clicks and takes its painting.
- `./scripts/verify` fails, naming the file and line, on a sample whose
  function or module is missing, named twice in its crate, or in a language
  other than rust.

The id names no file and no language, so a sample written in another language
later is a block with the same id, read from that language's tests.

## Paintings

`![The counter after two clicks](paint:beui-demo.a_counter_app_counts_its_clicks)`
shows `snapshots/beui-demo.a_counter_app_counts_its_clicks.paint`, the painting
a test took with `snapshot`; anything after a `#` is left for whatever renders
the docs. `./scripts/verify` fails on a painting the docs show that
snapshots/ does not hold once the tests have run. Nothing renders these yet,
so on GitHub they show as broken images.
