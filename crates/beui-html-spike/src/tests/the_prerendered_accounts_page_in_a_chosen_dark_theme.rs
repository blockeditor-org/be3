use super::*;

#[test]
fn the_prerendered_accounts_page_in_a_chosen_dark_theme() {
    beui::html::choose_theme(Some(beui::ThemeKind::Dark));
    BrowserSnapshot::new([1100, 720], &page()).snapshot("prerendered_accounts_page_dark.browser");
}
