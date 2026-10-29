use std::time::Duration;

use beui::native::WindowSize;
use beui_host::native::testing::{Showing, Stepped};
use beui::{native::Context, vec2};
use beui_host::{native::FreetypeFonts};

use super::*;

fn painting_of(name: &str, document: beui::native::Document) {
    let size = vec2(1100.0, 720.0);
    let window = WindowSize { size, scale: 1.0 };
    let context = Context::new(FreetypeFonts::default());
    let mut stepped = Stepped::new(context, Showing(document), window, None);
    for _ in 0..2 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
    stepped.advance(Duration::from_secs(1));
    let output = stepped.frame(Vec::new());
    let painted = beui::native::painting::painting(
        output.main(),
        size,
        1.0,
        output.main().window.appearance.clear_color,
        beui::native::painting::Punches::Shown,
    )
    .unwrap();
    let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
    paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
}

mod an_account_in_a_company_says_its_administrators_can_read_it;
