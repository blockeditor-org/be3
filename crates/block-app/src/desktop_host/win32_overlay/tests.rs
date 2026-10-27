use std::time::Duration;

use be_win32::{EntryKind, FocusError, Hotkey, HotkeyError};
use beui::testing::{Showing, Stepped};
use beui::{Context, FreetypeFonts, WindowSize, vec2};

use super::super::win32_map::{HotkeyState, focused_hint, hover_hint};
use super::*;

const SIZE: beui::Vec2 = vec2(640.0, 400.0);

fn painting(name: &str, notice: Option<SurfaceNotice>, hint: Option<Hint>, off_screen: Vec<Rect>) {
    let document = build(move || {
        let notice = create_memo(move || notice.clone());
        let hint = create_memo(move || hint.clone());
        let off_screen = create_memo(move || off_screen.clone());
        view! {
            <Root>
                <Frame fill=ColorRole::Accent>
                    <EntryOverlay notice hint off_screen />
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
    let output = stepped.frame(Vec::new());
    let painted = beui::painting::painting(
        output.main(),
        SIZE,
        1.0,
        output.main().window.appearance.clear_color,
        beui::painting::Punches::Shown,
    )
    .unwrap();
    let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
    paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
}

fn part(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(beui::pos2(left, top), beui::pos2(right, bottom))
}

mod a_focused_window_says_how_to_come_back_and_shades_what_is_off_screen;
mod a_hovered_window_says_a_click_uses_it;
