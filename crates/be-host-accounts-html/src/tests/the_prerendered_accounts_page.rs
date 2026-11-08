use super::*;

#[test]
fn the_prerendered_accounts_page() {
    BrowserSnapshot::new([1100, 720], &page()).snapshot("prerendered_accounts_page.browser");
}
