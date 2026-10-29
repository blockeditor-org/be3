use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use beui::native::WindowSize;
use beui_host::native::testing::{Showing, Stepped};
use beui::{Input, PointerButton, native::Context, vec2};
use beui_host::{native::FreetypeFonts};

use super::*;
use beui_host::native::build;

const SIZE: beui::Vec2 = vec2(480.0, 320.0);

type Pressed = Rc<RefCell<Vec<&'static str>>>;

fn stepped(notice: impl FnOnce() -> NodeId + 'static) -> Stepped {
    let document = build(move || {
        view! {
            <Root>
                <Frame fill=ColorRole::Background align=Align::Center>{notice()}</Frame>
            </Root>
        }
    });
    let window = WindowSize {
        size: SIZE,
        scale: 1.0,
    };
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        window,
        None,
    );
    for _ in 0..2 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
    stepped
}

fn click(stepped: &mut Stepped, test_id: &str) {
    let document = stepped.document();
    let button = document
        .find_test_id(test_id)
        .unwrap_or_else(|| panic!("{test_id} is shown"));
    let at = document.node_rect(button).unwrap().center();
    for pressed in [true, false] {
        stepped.frame(vec![
            Input::PointerMoved(at),
            Input::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
            },
        ]);
    }
    stepped.frame(Vec::new());
}

fn snapshot(stepped: &mut Stepped, name: &str) {
    stepped.advance(Duration::from_secs(1));
    let output = stepped.frame(Vec::new());
    let painted = beui::native::painting::painting(
        output.main(),
        SIZE,
        1.0,
        output.main().window.appearance.clear_color,
        beui::native::painting::Punches::Shown,
    )
    .unwrap();
    let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
    paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
}

fn pressing(pressed: &Pressed, what: &'static str) -> impl Fn() + 'static {
    let pressed = pressed.clone();
    move || pressed.borrow_mut().push(what)
}

mod a_block_another_build_saved_offers_reset_delete_and_try_anyway;
mod a_crashed_plugin_offers_a_reload;
