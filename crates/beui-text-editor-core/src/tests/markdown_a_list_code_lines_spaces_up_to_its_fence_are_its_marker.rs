use super::*;

/// A code block in a list outside a quote keeps each line's spaces up to its fence's column in
/// its marker, as a quoted one does, so its code lines up with its fence; spaces past the fence's
/// column are the code's own indentation.
#[test]
fn markdown_a_list_code_lines_spaces_up_to_its_fence_are_its_marker() {
    let source = "- Item\n\n  ```\n  let listed = 1;\n      nested();\n  ```\n\n- ```\n  on the marker's line\n  ```\n";
    let mut tester = EditorTester::with_language(source.as_bytes(), TextLanguage::Markdown);
    let highlight = tester.editor.highlight();
    let look = |line: &str| {
        let start = source.find(&format!("{line}\n")).unwrap();
        let look = highlight.line_look(start);
        (
            source[look.marker.clone()].to_owned(),
            look.depth,
            look.code_block,
        )
    };

    assert_eq!(look("  let listed = 1;"), ("  ".into(), 1, true));
    assert_eq!(look("      nested();"), ("  ".into(), 1, true));
    assert_eq!(look("  on the marker's line"), ("  ".into(), 1, true));
    let fences: Vec<_> = source
        .match_indices("  ```\n")
        .map(|(start, _)| {
            let look = highlight.line_look(start);
            (source[look.marker].to_owned(), look.code_block)
        })
        .collect();
    assert_eq!(fences, vec![("  ".to_owned(), true); 3]);
}
