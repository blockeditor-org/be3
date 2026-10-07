use super::*;

struct Blank;

impl Instance for Blank {
    fn connect(&mut self, _block_id: Uuid) {}

    fn input(&mut self, _region: &Region, _event: &InputEvent) {}

    fn update(&mut self, _region: &Region, _settings: Option<&mut Vec<u8>>) -> Frame {
        Frame::default()
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<block_plugin_api::SurfaceRect> {
        target.whole()
    }
}

fn touch(phase: TouchPhase) -> InputEvent {
    InputEvent::Touch {
        device: 0,
        finger: 1,
        phase,
        x: 10.0,
        y: 10.0,
        force: None,
    }
}

fn pans(session: &mut EditorSession) -> usize {
    session
        .outbound()
        .into_iter()
        .filter(|message| {
            matches!(
                message,
                Message::Editor(EditorMessage::ChangeView {
                    change: ViewChange::Pan { .. },
                    ..
                })
            )
        })
        .count()
}

#[test]
fn a_pan_waits_for_the_finger_to_lift_with_gesture_motion_off() {
    crate::motion::receive(Motion::Still);
    let mut session = EditorSession::adopt(
        EditorInstanceId(1),
        Box::new(Blank),
        EditorHost::new(Waker::default()),
    );

    session.input(EditorRegion::Frame, &touch(TouchPhase::Start));
    session.host().pan_view(vec2(5.0, 0.0));
    session.input(EditorRegion::Frame, &touch(TouchPhase::Move));
    session.host().pan_view(vec2(5.0, 0.0));

    assert_eq!(
        pans(&mut session),
        0,
        "nothing moves while the finger is down"
    );

    session.input(EditorRegion::Frame, &touch(TouchPhase::End));

    assert_eq!(pans(&mut session), 2, "lifting it sends the held pans");
    crate::motion::receive(Motion::Animated);
}
