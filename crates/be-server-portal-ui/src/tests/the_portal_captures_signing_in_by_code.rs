use super::*;

#[test]
fn the_portal_captures_signing_in_by_code() {
    let entering = Page {
        code_email: "alex@example.com".into(),
        ..page(Screen::EnterCode)
    };
    painting(entering, [390, 844]).snapshot("portal_enter_code.browser");
    let linked = Page {
        code_email: "alex@example.com".into(),
        email_code: "K7QM2RXT".into(),
        ..page(Screen::EnterCode)
    };
    painting(linked, [1100, 900]).snapshot("portal_enter_code_from_link.browser");
    let elsewhere = page(Screen::CodeElsewhere {
        code: "K7QM2RXT".into(),
    });
    painting(elsewhere, [390, 844]).snapshot("portal_code_elsewhere.browser");
    painting(page(Screen::Recover), [390, 844]).snapshot("portal_recover.browser");
}
