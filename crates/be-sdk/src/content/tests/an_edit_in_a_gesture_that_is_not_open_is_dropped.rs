use be_sdk_type_guest::editor::{FromEditor, Gesture};

use super::*;

/// What the editor sent its replica since last asked, but asks for its content.
fn sent(host: &EditorHost) -> Vec<FromEditor> {
    let mut sent = Vec::new();
    host.take_outgoing(|_, message| {
        if message != FromEditor::Resend {
            sent.push(message);
        }
        None
    });
    sent
}

/// An edit made in a gesture that is not open is dropped, never written as it happens: one
/// through a handle kept after its gesture committed, and one in a gesture begun while the
/// block was not editable, whose begin was never sent.
#[test]
fn an_edit_in_a_gesture_that_is_not_open_is_dropped() {
    let fixture = Fixture::new();
    let live = &fixture.projection;
    let drag = live.begin_tentative();
    live.commit_gesture(drag);
    live.pump();
    assert_eq!(
        sent(&fixture.host),
        [
            FromEditor::Gesture {
                id: drag.0,
                gesture: Gesture::BeginTentative,
            },
            FromEditor::Gesture {
                id: drag.0,
                gesture: Gesture::Commit,
            },
        ]
    );
    live.operate_in(drag, Checklist::set_text(fixture.first, "stale"));
    live.pump();
    assert_eq!(sent(&fixture.host), []);
    assert_eq!(fixture.text(fixture.first), "milk");

    fixture.host.set_editable(false);
    let unsent = live.begin_tentative();
    fixture.host.set_editable(true);
    assert!(!live.gesture_open(unsent));
    live.operate_in(unsent, Checklist::set_text(fixture.first, "unbegun"));
    live.pump();
    assert_eq!(sent(&fixture.host), []);
    assert_eq!(fixture.text(fixture.first), "milk");
}
