use be_lib_ui_types::clipboard::{ClipFormat, Uuid};

use super::*;

#[test]
fn rich_text_a_paste_between_two_rich_texts_keeps_bold_links_and_an_embed() {
    let (embedded, translated) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let mut source = EditorTester::rich(b"Title\nsay bold and link ");
    select(&mut source, 0..0);
    tool(&mut source, "heading1");
    select(&mut source, 10..14);
    tool(&mut source, "bold");
    select(&mut source, 19..23);
    source.execute(EditorCommand::SetMark(
        Name::new(schema::basic::LINK),
        Some(Value::text("https://a.test")),
    ));
    source.set_cursor(source.pos(24));
    source.execute(EditorCommand::InsertVoid(MarkSet::new([(
        Name::new(schema::basic::EMBED),
        Value::text(&embedded.to_string()),
    )])));
    let len = source.bytes().len();
    select(&mut source, 0..len);
    let copied = source.editor.copy(CopyMode::Copy);
    assert_eq!(copied.schema.as_deref(), Some("basic"));

    let mut item = copied.encode();
    assert_eq!(item.refs, vec![embedded], "the embed travels as a ref");
    item.refs = vec![translated];
    let pasted = item.decode::<TextFragment>().expect("the fragment decodes");

    let mut target = EditorTester::rich(b"x\nbefore after");
    select(&mut target, 2..14);
    tool(&mut target, "bold");
    target.set_cursor(target.pos(9));
    target.execute(EditorCommand::Paste(&pasted));
    assert_eq!(
        formatted(&target),
        format!(
            "x\n{{bold}}before {{}}Title\nsay {{bold}}bold{{}} and {{link=https://a.test}}link{{}} \
             {{embed={translated}}}\u{ef}\u{bf}\u{bc}{{bold}}after"
        ),
        "the marks come along, the pasted text takes none from around it, \
         and the embed names the translated block"
    );

    let mut line_start = EditorTester::rich(b"body");
    line_start.execute(EditorCommand::Paste(&pasted));
    assert!(
        formatted(&line_start).starts_with("[heading=1]Title\n"),
        "pasted at a line's start, a line keeps its props: {}",
        formatted(&line_start)
    );

    let foreign = TextFragment {
        schema: Some("another".to_owned()),
        ..pasted.clone()
    };
    let mut other = EditorTester::rich(b"");
    other.execute(EditorCommand::Paste(&foreign));
    assert_eq!(
        formatted(&other),
        "Title\nsay bold and link \u{ef}\u{bf}\u{bc}",
        "marks of another schema are dropped"
    );

    let mut plain = EditorTester::new(b"");
    plain.set_cursor(plain.pos(0));
    plain.execute(EditorCommand::Paste(&pasted));
    assert_eq!(
        plain.bytes(),
        "Title\nsay bold and link \u{fffc}".as_bytes(),
        "a plain text takes the bytes"
    );
}
