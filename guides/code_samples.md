# Code samples in the docs

A code sample in a Markdown file under guides/ can be the body of a function
some test runs, so it compiles and passes or the build says so:

````
```rust example=checklist.adding_an_item_puts_it_on_the_list
```
````

`example=CRATE.NAME` names `fn example_NAME` in the crate whose Cargo.toml
`[package]` name is CRATE, in any of its files (a crate nested inside its
directory is a crate of its own). `./scripts/verify` writes that function's
body into the block, dedented, so edit the function and never the block. The
docs fix reads the crates as the Rust autofixes leave them, so a sample shows
code formatted and without comments, as the crate holds it.

- A test calls the example, so it compiles, runs and is not dead code. Keep an
  example in the test file that calls it.
- What the sample leaves out is the example's parameters and whatever its test
  does before calling it: `fn example_recording_a_gesture(editor: &mut
  BeuiTest<ChecklistApp>)` shows the gesture and not the setup that built the
  editor.
- `./scripts/verify` fails, naming the file and line, on a sample whose
  function is missing, named twice in its crate, or in a language other than
  rust.

The id names no file and no language, so a sample written in another language
later is a block with the same id, read from that language's tests.
