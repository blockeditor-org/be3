use std::cell::RefCell;

use super::*;

struct Busy {
    frames: Rc<Cell<u32>>,
    scrolled: Rc<RefCell<Vec<Vec2>>>,
}

impl App for Busy {
    fn update(&mut self, context: &Context, _rect: Rect) {
        self.frames.set(self.frames.get() + 1);
        if self.frames.get() < 20 {
            context.request_repaint();
        }
        context.input(|input| {
            for event in &input.events {
                if let Event::Scroll(delta) = event {
                    self.scrolled.borrow_mut().push(*delta);
                }
            }
        });
    }
}

#[test]
fn a_driver_turns_the_wheel_and_answers_without_settling_when_asked() {
    let frames = Rc::new(Cell::new(0));
    let scrolled = Rc::new(RefCell::new(Vec::new()));
    let mut driven = Driven::with_app(
        Busy {
            frames: frames.clone(),
            scrolled: scrolled.clone(),
        },
        false,
    );

    driven
        .ask(&["--no-settle", "move", "10,10"])
        .expect("the move is answered");
    assert!(
        frames.get() < 5,
        "the answer came before the app settled, after {} frames",
        frames.get()
    );
    driven.ask(&["settle"]).expect("the app settles");
    assert!(frames.get() >= 20);

    driven
        .ask(&["wheel", "10,10", "2", "-1"])
        .expect("the wheel turns");
    assert_eq!(
        *scrolled.borrow(),
        [vec2(40.0, -40.0), vec2(0.0, -40.0)],
        "two ticks down, one right, a frame each"
    );
}
