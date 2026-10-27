use super::*;
use beui_view::types::input::{Keystroke, TouchId};
use beui_view::types::vec2;

mod a_circle_on_a_letter_types_it_capital;
mod a_held_key_repeats_until_it_is_let_go;
mod a_latched_modifier_lasts_one_key_and_caps_lock_lasts_until_it_is_let_go;
mod a_swipe_toward_a_corner_types_what_the_corner_shows;
mod compose_builds_a_letter_from_the_keys_after_it;
mod composing_shows_what_each_key_would_make_and_fades_the_rest;
mod ctrl_turns_the_next_letter_into_a_chord;
mod dragging_along_space_moves_the_cursor_a_letter_at_a_time;
mod dragging_left_from_backspace_selects_words_and_letting_go_deletes_them;
mod fn_turns_digits_into_function_keys;
mod superscript_changes_the_next_key_only;

const RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(400.0, 160.0),
};

struct Typist {
    engine: Engine,
    now: Instant,
    finger: u64,
}

impl Typist {
    fn new() -> Self {
        let mut engine = Engine::default();
        let layout = engine.layout();
        engine.set_key_size(vec2(
            RECT.width() / layout.width(),
            RECT.height() / layout.height(),
        ));
        Self {
            engine,
            now: Instant::now(),
            finger: 0,
        }
    }

    fn center(&self, value: Value) -> Pos2 {
        let layout = self.engine.layout();
        let at = layout.find(value).expect("a key shows that value");
        layout
            .key_rect(RECT, at)
            .expect("the key is laid out")
            .center()
    }

    fn pointer(&mut self) -> PointerId {
        self.finger += 1;
        PointerId::Touch(TouchId {
            device: 0,
            finger: self.finger,
        })
    }

    fn down(&mut self, at: Pos2) -> PointerId {
        let id = self.pointer();
        let key = Engine::hit(self.engine.layout(), RECT, at).expect("a key is under the finger");
        self.engine.press(id, key, at, self.now);
        id
    }

    fn path(&mut self, value: Value, moves: &[Vec2]) {
        let start = self.center(value);
        let id = self.down(start);
        for delta in moves {
            self.now += Duration::from_millis(10);
            self.engine.moved(id, start + *delta, self.now);
        }
        self.engine.released(id, self.now);
    }

    fn tap(&mut self, value: Value) {
        self.path(value, &[]);
    }

    fn swipe(&mut self, value: Value, delta: Vec2) {
        self.path(value, &[delta * 0.5, delta]);
    }

    fn typed(&mut self) -> String {
        summary(&self.engine.take_events())
    }
}

fn summary(events: &[Input]) -> String {
    let mut out = String::new();
    let mut modifiers = Modifiers::NONE;
    for event in events {
        match event {
            Input::Modifiers(held) => modifiers = *held,
            Input::Text(text) => out.push_str(text),
            Input::Key(Keystroke {
                pressed: true,
                text: Some(text),
                ..
            }) if !modifiers.command() => out.push_str(text),
            Input::Key(Keystroke {
                key: Some(key),
                pressed: true,
                ..
            }) if !key.is_modifier() => {
                let mut chord = String::new();
                for (held, name) in [
                    (modifiers.ctrl, "ctrl+"),
                    (modifiers.alt, "alt+"),
                    (modifiers.shift, "shift+"),
                    (modifiers.logo, "super+"),
                ] {
                    if held {
                        chord.push_str(name);
                    }
                }
                out.push_str(&format!("[{chord}{key:?}]"));
            }
            _ => {}
        }
    }
    out
}
