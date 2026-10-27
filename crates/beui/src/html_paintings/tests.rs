use crate::reactive::*;
use crate::styled::*;
use paint_snapshot::BrowserSnapshot;

fn painting(size: [u32; 2], root: impl FnOnce() -> crate::primitives::Root) -> BrowserSnapshot {
    let markup = crate::html::render_to_string(root);
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}body {{ margin: 0; height: 100vh; font: 14px sans-serif; }}</style></head><body>{markup}</body></html>",
        crate::html::stylesheet()
    );
    BrowserSnapshot::new(size, &page)
}

mod a_long_modal_sheet_on_the_html_core;
mod a_styled_dialog_on_the_html_core;
mod pictures_wheels_and_themed_drawings_on_the_html_core;
mod styled_buttons_and_a_checkbox_on_the_html_core;
mod styled_list_rows_on_the_html_core;
mod the_theme_follows_the_chosen_theme_after_the_first_component_that_asked_for_it_goes;
