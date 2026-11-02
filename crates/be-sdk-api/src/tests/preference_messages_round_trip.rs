use super::*;

#[test]
fn preference_messages_round_trip() {
    for theme in ThemeKind::ALL {
        for motion in Motion::ALL {
            for canvas_touch in CanvasTouch::ALL {
                let message = Preferences {
                    theme,
                    motion,
                    canvas_touch,
                };
                round_trips(&message);
            }
        }
    }
}
