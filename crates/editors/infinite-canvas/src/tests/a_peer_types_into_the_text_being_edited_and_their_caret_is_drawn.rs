use super::*;
use crate::presence::{CanvasCursor, TextCaret};
use block_editor_beui::PeerPresence;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use block_editor_beui::be_block::presence::{PresenceColor, PresenceKind};
use block_editor_beui::beui::pos2;

fn retyped(label: &CanvasEntity, text: &str) -> CanvasEntity {
    let mut label = label.clone();
    if let CanvasEntityKind::Text { text: held, .. } = &mut label.kind {
        *held = text.to_owned();
    }
    label
}

#[test]
fn a_peer_types_into_the_text_being_edited_and_their_caret_is_drawn() {
    let mut label = entity(Uuid::from_u128(1));
    label.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(220.0, 40.0), 0.0);
    label.kind = CanvasEntityKind::Text {
        text: "Hello".to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: String::new(),
    };
    let mut editor = editor(std::slice::from_ref(&label));
    editor.presence_visible(true);
    editor.run();
    let field = format!("infinite-canvas.text.{}", label.id);
    let resting = editor.rect_of(&field);
    editor.double_click_at(pos2(resting.right() - 2.0, resting.center().y));
    editor.run();

    apply(
        &mut editor,
        InfiniteCanvasOperation::Update {
            entities: vec![retyped(&label, "Oh, Hello")],
        },
    );
    editor.run();
    let theirs = CanvasCursor {
        pointer: None,
        selection: vec![label.id],
        text: Some(TextCaret {
            entity: label.id,
            anchor: 0,
            focus: 3,
        }),
        color: PresenceColor::Purple,
    };
    editor.set_peers(
        None,
        vec![PeerPresence {
            client: 7,
            kind: CanvasCursor::ID,
            value: serde_json::to_vec(&theirs).unwrap(),
        }],
    );
    editor.run();
    editor.take_shown_presence();

    editor.text("!");
    editor.run();

    let CanvasEntityKind::Text { text, .. } = &entities(&editor)[0].kind else {
        panic!("the entity is still text");
    };
    assert_eq!(
        text, "Oh, Hello!",
        "the peer's words are kept and typing stays at the caret"
    );
    let ours: CanvasCursor = editor
        .take_shown_presence()
        .iter()
        .rev()
        .find(|shown| shown.kind == CanvasCursor::ID)
        .and_then(|shown| serde_json::from_slice(shown.value.as_ref()?).ok())
        .expect("the canvas shows its caret to its peers");
    assert_eq!(
        ours.text,
        Some(TextCaret {
            entity: label.id,
            anchor: 10,
            focus: 10,
        })
    );
    editor.snapshot("a_peer_types_into_the_text_being_edited_and_their_caret_is_drawn");
}
