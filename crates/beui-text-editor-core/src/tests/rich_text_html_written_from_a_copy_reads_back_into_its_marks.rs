use super::*;

#[test]
fn rich_text_html_written_from_a_copy_reads_back_into_its_marks() {
    let mut source = EditorTester::rich(b"Notes\nsay <bold> & link\none\ntwo");
    select(&mut source, 0..0);
    tool(&mut source, "heading1");
    select(&mut source, 10..16);
    tool(&mut source, "bold");
    select(&mut source, 19..23);
    source.execute(EditorCommand::SetMark(
        Name::new(schema::basic::LINK),
        Some(Value::text("https://a.test/\"x\"")),
    ));
    select(&mut source, 24..31);
    tool(&mut source, "bullet");
    let len = source.bytes().len();
    select(&mut source, 0..len);
    let copied = source.editor.copy(CopyMode::Copy);
    let schema = source.editor.schema();
    let html = schema.to_html(&copied).expect("a rich copy writes html");
    assert_eq!(
        html,
        "<h1>Notes</h1><p>say <b>&lt;bold&gt;</b> &amp; \
         <a href=\"https://a.test/&quot;x&quot;\">link</a></p><ul><li>one</li><li>two</li></ul>"
    );

    let mut target = EditorTester::rich(b"");
    let read = schema.from_html(&html).expect("and reads it back");
    target.execute(EditorCommand::Paste(&read));
    assert_eq!(formatted(&target), formatted(&source));
}
