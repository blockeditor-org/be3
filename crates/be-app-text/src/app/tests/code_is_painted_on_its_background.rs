use super::editor;

#[test]
fn code_is_painted_on_its_background() {
    let mut editor = editor(
        "Inline `abc   def` code.\n\n```rust\nfn main() {\n    let x = 1;\n\n}\n```\n\nAfter.\n",
    );
    editor.run();

    editor.snapshot("code_is_painted_on_its_background");
}
