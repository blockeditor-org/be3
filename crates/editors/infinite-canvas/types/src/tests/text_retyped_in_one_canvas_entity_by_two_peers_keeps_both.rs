use super::*;
use crate::{Canvas, CanvasContent, InfiniteCanvasOperation};

fn text_of(content: &CanvasContent) -> String {
    content.root().entities()[0]
        .text
        .to_str_lossy()
        .into_owned()
}

#[test]
fn text_retyped_in_one_canvas_entity_by_two_peers_keeps_both() {
    let mut note = rectangle();
    note.text = "hello world".into();
    let base = canvas_run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: Box::new(note.clone()),
            pixels: None,
        },
    );
    assert_eq!(text_of(&base), "hello world");

    let ours = Canvas::retext(&base, 1, note.id, "hello, world").unwrap();
    let theirs = Canvas::retext(&base, 2, note.id, "hello world!").unwrap();
    let one_way = edited(&base, [ours.clone(), theirs.clone()]);
    let other_way = edited(&base, [theirs, ours]);
    assert_eq!(text_of(&one_way), "hello, world!");
    assert_eq!(text_of(&other_way), "hello, world!");

    let mut stale = note;
    stale.transform.center.x = 40.0;
    let moved = canvas_run(
        &one_way,
        InfiniteCanvasOperation::Update {
            entities: vec![stale],
        },
    );
    assert_eq!(
        text_of(&moved),
        "hello, world!",
        "an update made from an older copy of the entity leaves its text alone"
    );
}
