use super::*;

#[test]
fn a_fenced_block_is_highlighted_in_the_language_it_names() {
    let mut editor = editor(concat!(
        "```rs\nfn main() {\n    let answer: u32 = 42; // a comment\n}\n```\n\n",
        "```toml\n[package]\nname = \"text\"\n```\n\n",
        "```zig\nconst x: u8 = 1;\n```\n\n",
        "```unknown\nfn main() {}\n```\n",
    ));
    editor.run();

    editor.snapshot("a_fenced_block_is_highlighted_in_the_language_it_names");
}
