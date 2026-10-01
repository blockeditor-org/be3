use super::*;

#[test]
fn markdown_windows_follow_edits_inside_them() {
    let mut text = String::new();
    for line in 0..400 {
        text.push_str(&format!("para {line} with *some* words\n"));
        if line % 7 == 0 {
            text.push_str("\n| a | b |\n|---|---|\n| 1 | 2 |\n\n");
        }
        if line % 45 == 0 {
            text.push_str("\n```\n# not a heading\n- [ ] not a task\n```\n\n");
        }
    }
    let mut tester = EditorTester::with_language(text.as_bytes(), TextLanguage::Markdown);
    let edits: [(&str, &[u8]); 5] = [
        ("para 120 ", b"**bold "),
        ("para 121 ", b"\n\n# Heading\n"),
        ("para 200 ", b"```\n"),
        ("para 300 ", b"| x | y |\n|---|---|\n"),
        ("para 50 ", b"\n"),
    ];
    for (at, inserted) in edits {
        let highlight = tester.editor.highlight();
        let bytes = tester.bytes();
        for index in (0..bytes.len()).step_by(512) {
            highlight.style_at(index);
        }
        let index = String::from_utf8(bytes).unwrap().find(at).unwrap();
        tester.set_cursor(tester.pos(index));
        tester.execute(EditorCommand::InsertText(inserted));

        let edited = tester.bytes();
        let after = tester.editor.highlight();
        let fresh = EditorTester::with_language(&edited, TextLanguage::Markdown)
            .editor
            .highlight();
        for index in (0..edited.len()).step_by(31) {
            assert_eq!(
                after.style_at(index),
                fresh.style_at(index),
                "after inserting at {at:?}, the style at {index} differs"
            );
        }
        assert_eq!(after.markdown_tables(), fresh.markdown_tables());
        assert_eq!(
            after.in_code_block(index..index + 1),
            fresh.in_code_block(index..index + 1)
        );
    }
}
