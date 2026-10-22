use super::*;

struct Busy {
    frames: Signal<u32>,
    scrolled: Signal<Vec<Vec2>>,
}

impl App for Busy {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let frames = self.frames.update(|frames| {
            *frames += 1;
            *frames
        });
        if frames < 20 {
            context.request_repaint();
        }
        context.input(|input| {
            for event in &input.events {
                if let Event::Scroll(delta) = event {
                    self.scrolled.update(|scrolled| scrolled.push(*delta));
                }
            }
        });
    }
}

#[test]
fn a_driver_turns_the_wheel_and_answers_without_settling_when_asked() {
    let _root = crate::reactive::RootScope::new();
    let frames = create_signal(0);
    let scrolled = create_signal(Vec::new());
    let mut driven = stepped(Busy { frames, scrolled });

    driven
        .ask(&["--no-settle", "move", "10,10"])
        .expect("the move is answered");
    assert!(
        frames.get_untracked() < 5,
        "the answer came before the app settled, after {} frames",
        frames.get_untracked()
    );
    driven.ask(&["settle"]).expect("the app settles");
    assert!(frames.get_untracked() >= 20);

    driven
        .ask(&["wheel", "10,10", "2", "-1"])
        .expect("the wheel turns");
    assert_eq!(
        scrolled.get_untracked(),
        [vec2(40.0, -40.0), vec2(0.0, -40.0)],
        "two ticks down, one right, a frame each"
    );
}
