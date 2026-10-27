use paint_snapshot::BrowserSnapshot;

fn page() -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}html, body {{ margin: 0; height: 100%; }}</style></head><body>{}</body></html>",
        beui::html::BASE_STYLES,
        crate::markup(),
    )
}

#[test]
fn the_prerendered_accounts_page() {
    BrowserSnapshot::new([1100, 720], &page()).snapshot("prerendered_accounts_page.browser");
}

#[test]
fn the_prerendered_accounts_page_in_a_chosen_dark_theme() {
    beui::html::choose_theme(Some(beui::ThemeKind::Dark));
    BrowserSnapshot::new([1100, 720], &page()).snapshot("prerendered_accounts_page_dark.browser");
}
