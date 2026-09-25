use super::*;
use crate::presence::CanvasCursor;
use block_editor_plugin::PeerPresence;
use block_editor_plugin::be_block::canvas::CanvasColor;
use block_editor_plugin::be_block::presence::{PresenceColor, PresenceKind};

#[test]
fn selections_are_shared_with_peers_and_theirs_are_drawn() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 120.0), 0.0);
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    });
    let mut editor = editor(std::slice::from_ref(&rectangle));
    editor.presence_visible(true);
    editor.run();

    editor.click(&format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.run();

    let shown = editor.host.take_shown_presence();
    let cursor: CanvasCursor = shown
        .iter()
        .rev()
        .find(|shown| shown.kind == CanvasCursor::ID)
        .and_then(|shown| serde_json::from_slice(shown.value.as_ref()?).ok())
        .expect("the canvas shows its cursor to its peers");
    assert_eq!(cursor.selection, [rectangle.id]);

    let theirs = CanvasCursor {
        pointer: Some(CanvasPoint::new(40.0, 30.0)),
        selection: vec![rectangle.id],
        color: PresenceColor::Purple,
    };
    editor.host.set_peers(
        None,
        vec![PeerPresence {
            client: 7,
            kind: CanvasCursor::ID,
            value: serde_json::to_vec(&theirs).unwrap(),
        }],
    );
    editor.run();
    editor.run();

    editor.snapshot("selections_are_shared_with_peers_and_theirs_are_drawn");
}
