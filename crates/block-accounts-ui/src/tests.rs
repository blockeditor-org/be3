use std::time::Duration;

use beui::native::WindowSize;
use beui::{native::Context, vec2};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

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
    stepped.frame(Vec::new());
    stepped.snapshot(name);
}

mod an_account_in_a_company_says_its_administrators_can_read_it;
