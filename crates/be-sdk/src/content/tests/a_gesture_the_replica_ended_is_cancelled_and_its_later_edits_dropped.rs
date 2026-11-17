use std::cell::RefCell;

use be_sdk_type_guest::editor::{FromEditor, Gesture, ToEditor};

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

/// Two drags held at once go under their own ids. The replica ends one: the projection answers
/// with a cancel for it alone, tells the editor, and sends nothing more of it, while the other
/// drag carries on and commits.
#[test]
fn a_gesture_the_replica_ended_is_cancelled_and_its_later_edits_dropped() {
    let fixture = Fixture::new();
    let live = &fixture.projection;
    let ended = Rc::new(RefCell::new(Vec::new()));
    let _ending = live.on_gesture_ended({
        let ended = Rc::clone(&ended);
        move |gesture| ended.borrow_mut().push(gesture)
    });
    let drag = live.begin_tentative();
    let other = live.begin_tentative();
    assert_ne!(drag, other);
    live.operate_in(drag, Checklist::set_text(fixture.first, "dragged"));
    live.pump();
    let begun = |gesture: super::super::ContentGesture| FromEditor::Gesture {
        id: gesture.0,
        gesture: Gesture::BeginTentative,
    };
    assert_eq!(
        sent(&fixture.host),
        [
            begun(drag),
            begun(other),
            FromEditor::Operate {
                gesture: Some(drag.0),
                operation: ChecklistContent::encode_operation(&Checklist::set_text(
                    fixture.first,
                    "dragged"
                )),
            },
        ]
    );

    fixture.host.receive_from_replica(
        None,
        ToEditor::GestureEnded {
            id: drag.0,
            cancelled: true,
        },
    );
    live.pump();
    assert_eq!(*ended.borrow(), [drag]);
    assert!(!live.gesture_open(drag) && live.gesture_open(other));
    assert_eq!(
        sent(&fixture.host),
        [FromEditor::Gesture {
            id: drag.0,
            gesture: Gesture::Cancel,
        }]
    );

    live.operate_in(drag, Checklist::set_text(fixture.first, "later"));
    live.commit_gesture(drag);
    live.commit_gesture(other);
    live.pump();
    assert_eq!(
        sent(&fixture.host),
        [FromEditor::Gesture {
            id: other.0,
            gesture: Gesture::Commit,
        }]
    );
}
