use super::*;
use crate::presence::{CanvasCursor, TextCaret};
use block_editor_beui::PeerPresence;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use block_editor_beui::be_block::presence::{PresenceColor, PresenceKind};
use block_editor_beui::beui::pos2;
use block_editor_beui::beui::unstyled;

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

    peer_types(&mut editor, label.id, 0, "Oh, ");
    editor.run();
    let node = editor
        .document()
        .find_test_id(&field)
        .expect("the text has a field");
    let area = unstyled::text_input_state(editor.document(), node);
    let (anchor, focus) = area.with_core(|core| (core.position(0), core.position(3)));
    let theirs = CanvasCursor {
        pointer: None,
        selection: vec![label.id],
        text: Some(TextCaret {
            entity: label.id,
            anchor,
            focus,
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
    editor.text("!");
    editor.run();

    let CanvasEntityKind::Text { text, .. } = &entities(&editor)[0].kind else {
        panic!("the entity is still text");
    };
    assert_eq!(
        text, "Oh, Hello!",
        "the peer's words are kept and typing stays at the caret"
    );
    let ours = editor
        .take_shown_presence()
        .iter()
        .rev()
        .find(|shown| shown.kind == CanvasCursor::ID)
        .and_then(|shown| serde_json::from_slice::<CanvasCursor>(shown.value.as_ref()?).ok())
        .and_then(|cursor| cursor.text)
        .expect("the canvas shows its caret to its peers");
    assert_eq!(ours.entity, label.id);
    assert_eq!(
        area.with_core(|core| core.position_index(ours.focus)),
        Some(10),
        "the caret it shows is after what was typed"
    );
    editor.snapshot("a_peer_types_into_the_text_being_edited_and_their_caret_is_drawn");
}
