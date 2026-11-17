use beui::reactive::view;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Fixture;

mod hosts;
mod live;

#[wasm_bindgen]
pub fn start(fixture: String) -> Result<(), JsValue> {
    be_wasm_threads::initialize_page();
    beui::html::run("app", move || {
        view! {
            <Fixture name={fixture} />
        }
    })
    .map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub async fn automate(words: Vec<String>) -> Result<String, JsValue> {
    beui::html::automate(words)
        .await
        .map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub fn checks() -> String {
    be_wasm_threads::initialize_page();
    live::run()
}

/// Whether the blank `BlankFixture` shows is as tall as its line, `width` wide and has its glyph
/// on the line: the box a mark over it fills, which a blank with no font size would leave empty.
pub(crate) fn blank_box(width: f32) -> String {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return "Blank box unseen".to_owned();
    };
    let found = |css: &str| document.query_selector(css).ok().flatten();
    let (Some(blank), Some(glyph)) = (found(".beui-blank"), found(".beui-blank .beui-glyph")) else {
        return "Blank box unseen".to_owned();
    };
    let Some(text) = blank.parent_element() else {
        return "Blank box unseen".to_owned();
    };
    let (blank, glyph, text) = (
        blank.get_bounding_client_rect(),
        glyph.get_bounding_client_rect(),
        text.get_bounding_client_rect(),
    );
    let tall = blank.height() >= text.height() * 0.6;
    let wide = (blank.width() - f64::from(width)).abs() <= 0.75;
    let on_line = glyph.top() >= text.top() - 1.0 && glyph.bottom() <= text.bottom() + 1.0;
    match (tall, wide, on_line) {
        (true, true, true) => "Blank box tall and wide, its glyph on its line".to_owned(),
        _ => format!(
            "Blank box {:.1}x{:.1} at {:.1}, glyph at {:.1}..{:.1}, in a line {:.1}..{:.1}",
            blank.width(),
            blank.height(),
            blank.top(),
            glyph.top(),
            glyph.bottom(),
            text.top(),
            text.bottom()
        ),
    }
}
