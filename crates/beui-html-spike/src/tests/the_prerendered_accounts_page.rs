use paint_snapshot::BrowserSnapshot;

fn page(scheme: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}html, body {{ margin: 0; height: 100%; }} .beui-root {{ color-scheme: {scheme}; }}</style></head><body>{}</body></html>",
        beui::html::BASE_STYLES,
        crate::markup(),
    )
}

#[test]
fn the_prerendered_accounts_page() {
    BrowserSnapshot::new([1100, 720], &page("light")).snapshot("prerendered_accounts_page.browser");
}

#[test]
fn the_prerendered_accounts_page_in_the_dark() {
    BrowserSnapshot::new([1100, 720], &page("dark")).snapshot("prerendered_accounts_page_dark.browser");
}
