use be_sdk_test::DocumentTest;
use beui::{Input, Key, Vec2, pos2};

use super::*;

const WIDE: Vec2 = Vec2::new(1100.0, 800.0);

fn suspend() -> LockAction {
    LockAction {
        label: "Suspend".to_owned(),
        glyph: ICON_MOON.to_owned(),
    }
}

fn press(test: &mut DocumentTest, key: Key) {
    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| {
                Input::Key(beui::Keystroke {
                    key: Some(key),
                    pressed,
                    ..Default::default()
                })
            })
            .collect(),
    );
    test.frame(Vec::new());
}

mod it_stays_locked_until_its_password_is_typed;
mod it_scrolls_on_a_window_too_short_for_it;
mod its_actions_are_told_apart_by_place;
