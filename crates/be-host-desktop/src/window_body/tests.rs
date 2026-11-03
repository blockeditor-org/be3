use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use beui::{native::Context, native::WindowSize, vec2};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

use super::*;
use beui_host::native::build;

const SIZE: Vec2 = vec2(640.0, 400.0);

mod a_body_too_small_for_its_window_says_so;
mod a_window_drawn_past_its_body_fades_along_the_clipped_edges;
mod a_window_that_is_not_responding_offers_to_wait_or_force_close;

fn status(drawn: Vec2, min_size: Vec2, responding: bool) -> WindowStatus {
    WindowStatus {
        name: "Drawing".to_owned(),
        title: "Drawing \u{2014} Untitled".to_owned(),
        responding,
        size: drawn,
        drawn,
        min_size,
        ..WindowStatus::default()
    }
}

struct Shown {
    stepped: Stepped<Showing>,
    status: Signal<Option<WindowStatus>>,
    closed: Rc<Cell<u32>>,
}

fn shown(initial: WindowStatus) -> Shown {
    let closed = Rc::new(Cell::new(0));
    let mut held = None;
    let counted = closed.clone();
    let document = build(|| {
        let signal = create_signal(Some(initial));
        held = Some(signal);
        let status = create_memo(move || signal.get());
        let on_force_close = ClickCallback::new(move || counted.set(counted.get() + 1));
        view! {
            <Root>
                <WindowBody status on_force_close>
                    <Frame fill=ColorRole::Accent />
                </WindowBody>
            </Root>
        }
    });
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        WindowSize {
            size: SIZE,
            scale: 1.0,
        },
        None,
    );
    settle(&mut stepped);
    Shown {
        stepped,
        status: held.expect("the status is made while the document is built"),
        closed,
    }
}

fn settle(stepped: &mut Stepped<Showing>) {
    for _ in 0..3 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
}

impl Shown {
    fn has(&mut self, test_id: &str) -> bool {
        self.stepped
            .frame(Vec::new())
            .test_id_rect(test_id)
            .is_some()
    }
}
