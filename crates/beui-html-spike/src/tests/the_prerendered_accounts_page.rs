use paint_snapshot::BrowserSnapshot;

#[test]
fn the_prerendered_accounts_page() {
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}html, body {{ margin: 0; height: 100%; }}</style></head><body>{}</body></html>",
        beui::html::BASE_STYLES,
        crate::markup(),
    );
    BrowserSnapshot::new([1100, 720], &page).snapshot("prerendered_accounts_page.browser");
}
