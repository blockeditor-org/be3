use paint_snapshot::BrowserSnapshot;

mod the_prerendered_accounts_page;
mod the_prerendered_accounts_page_in_a_chosen_dark_theme;

fn page() -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}</style></head><body>{}</body></html>",
        beui::html::stylesheet("."),
        crate::markup(),
    )
}
