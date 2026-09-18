use super::*;

#[test]
fn text_merges_line_by_line_and_marks_real_conflicts() {
    let base = TextContent::from("one\ntwo\nthree\n");
    let ours = TextContent::from("ONE\ntwo\nthree\n");
    let theirs = TextContent::from("one\ntwo\nTHREE\n");

    let merged = TextContent::merge3(&base, &ours, &theirs);
    assert!(merged.is_clean());
    assert_eq!(merged.value().text(), "ONE\ntwo\nTHREE\n");

    let ours = TextContent::from("one\nours\nthree\n");
    let theirs = TextContent::from("one\ntheirs\nthree\n");
    let merged = TextContent::merge3(&base, &ours, &theirs);
    let MergeResult::Conflicted { value, conflicts } = merged else {
        panic!("two rewrites of one line merged silently");
    };
    assert_eq!(conflicts, 1);
    assert_eq!(
        value.text(),
        "one\n<<<<<<< local\nours\n=======\ntheirs\n>>>>>>> remote\nthree\n"
    );

    let retyped = TextContent::from("one\ntwo\nthree\n").with_language(TextLanguage::Rust);
    let merged = TextContent::merge3(&base, &retyped, &base);
    assert!(merged.is_clean());
    assert_eq!(merged.value().language(), TextLanguage::Rust);

    let merged = TextContent::merge3(&base, &base, &retyped);
    assert_eq!(merged.value().language(), TextLanguage::Rust);
    assert_eq!(base.name().as_deref(), Some("one"));
}
