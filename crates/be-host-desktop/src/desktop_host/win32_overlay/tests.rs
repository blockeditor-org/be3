use std::time::Duration;

use beui::{native::Context, native::WindowSize, vec2};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

use super::*;
use beui_host::native::build;

const SIZE: beui::Vec2 = vec2(640.0, 400.0);

fn painting(name: &str, secure: bool, off_screen: Vec<Rect>) {
    let document = build(move || {
        let secure = create_memo(move || secure);
        let off_screen = create_memo(move || off_screen.clone());
        view! {
            <Root>
                <Frame fill=ColorRole::Accent>
                    <EntryOverlay secure off_screen />
                </Frame>
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
    for _ in 0..3 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
    stepped.frame(Vec::new());
    stepped.snapshot(name);
}

fn part(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(beui::pos2(left, top), beui::pos2(right, bottom))
}

mod a_display_on_the_secure_desktop_says_to_answer_it;
mod a_window_shades_what_is_off_screen;
