use super::*;
use crate::canvas::{Canvas, CanvasContent, CanvasTextStyle, InfiniteCanvasOperation};

fn text_of(content: &CanvasContent) -> String {
    match &content.root().entities()[0].kind {
        CanvasEntityKind::Text { text, .. } => text.clone(),
        other => panic!("the entity is still text, not {other:?}"),
    }
}

#[test]
fn text_typed_into_one_canvas_text_by_two_peers_keeps_both() {
    let mut note = rectangle();
    note.kind = CanvasEntityKind::Text {
        text: "hello world".to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: String::new(),
    };
    let base = canvas_run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: note.clone(),
        },
    );
    assert_eq!(text_of(&base), "hello world");

    let ours = Canvas::replace_text(&base, 1, note.id, 5..5, b",").unwrap();
    let theirs = Canvas::replace_text(&base, 2, note.id, 11..11, b"!").unwrap();
    let one_way = edited(&base, [ours.clone(), theirs.clone()]);
    let other_way = edited(&base, [theirs, ours]);
    assert_eq!(text_of(&one_way), "hello, world!");
    assert_eq!(text_of(&other_way), "hello, world!");

    let mut stale = note;
    stale.transform.center.x = 40.0;
    let restyled = canvas_run(
        &one_way,
        InfiniteCanvasOperation::Update {
            entities: vec![stale],
        },
    );
    assert_eq!(
        text_of(&restyled),
        "hello, world!",
        "an update made from an older copy of the entity leaves its text alone"
    );
}
