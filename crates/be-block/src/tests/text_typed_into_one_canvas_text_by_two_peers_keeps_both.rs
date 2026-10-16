use super::*;
use crate::canvas::{Canvas, CanvasContent, CanvasTextStyle, InfiniteCanvasOperation};

fn typed(entity: &CanvasEntity, text: &str) -> InfiniteCanvasOperation {
    let mut entity = entity.clone();
    entity.kind = CanvasEntityKind::Text {
        text: text.to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: String::new(),
    };
    InfiniteCanvasOperation::Update {
        entities: vec![entity],
    }
}

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

    let ours = Canvas::edit_for(&base, 1, &typed(&note, "hello, world"));
    let theirs = Canvas::edit_for(&base, 2, &typed(&note, "hello world!"));

    let one_way = edited(&base, [ours.clone(), theirs.clone()]);
    let other_way = edited(&base, [theirs, ours]);

    assert_eq!(text_of(&one_way), "hello, world!");
    assert_eq!(text_of(&other_way), "hello, world!");
}
