use super::*;
use crate::input::BackGesture;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn interrupting_a_sheet_bounce_leaves_no_displacement() {
    for interruption in 0..4 {
        let _root = RootScope::new();
        let sheet = NodeRef::new();
        let closed = create_signal(0);
        let document = sheet_of_rows(0.9, sheet, closed);
        let mut harness = Harness::sized(document, vec2(400.0, 600.0));
        harness.settle();
        let handle = harness.find("sheet.handle");
        let top = harness.rect(handle).top();
        harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 200.0));
        harness.settle();
        harness.finger_drag(&[pos2(200.0, 200.0), pos2(200.0, 260.0), pos2(200.0, 320.0)]);
        let mut bounced = false;
        for _ in 0..120 {
            harness.frame(Vec::new());
            if harness.rect(handle).top() > top + 4.0 {
                bounced = true;
                break;
            }
        }
        assert!(bounced);
        match interruption {
            0 => harness.scroll_at(pos2(200.0, 300.0), vec2(0.0, -20.0), Modifiers::NONE),
            1 => {
                harness.frame(vec![Input::Back(BackGesture::Started {
                    edge: crate::input::BackEdge::Left,
                })]);
                harness.frame(vec![Input::Back(BackGesture::Progressed(0.2))]);
                harness.frame(vec![Input::Back(BackGesture::Cancelled)]);
            }
            2 => {
                harness.touch(TouchPhase::Start, pos2(200.0, 300.0));
                harness.touch(TouchPhase::Move, pos2(200.0, 310.0));
                harness.touch(TouchPhase::Cancel, pos2(200.0, 310.0));
            }
            _ => harness.context().set_preference_layer(
                PreferenceSource::System,
                PreferenceLayer {
                    motion: Some(Motion::Instant),
                    ..PreferenceLayer::EMPTY
                },
            ),
        }
        harness.settle();
        assert_eq!(harness.rect(handle).top(), top);
        assert_eq!(closed.get_untracked(), 0);
    }
}
