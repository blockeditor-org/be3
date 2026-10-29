use be_portal::{Account, Page, Problem, Screen, Session, SessionKind};
use paint_snapshot::BrowserSnapshot;

use crate::render;

mod a_page_renders_the_same_twice_and_escapes_what_it_shows;
mod forms_escape_their_code_and_leave_validation_to_the_server;
mod the_portal_captures_account_management_and_approval;
mod the_portal_captures_errors_and_server_policy;
mod the_portal_captures_mail_and_completion_pages;
mod the_portal_forms_are_captured_on_desktop_and_phone;

fn page(screen: Screen) -> Page {
    Page {
        screen,
        csrf: "snapshot-form-token".into(),
        mail_token: "snapshot-mail-token".into(),
        allow_registration: true,
        ..Page::default()
    }
}

fn signed_in(screen: Screen) -> Page {
    Page {
        account: Some(Account {
            display_name: "Alex".into(),
            email: "alex@example.com".into(),
        }),
        email: "alex@example.com".into(),
        display_name: "Alex".into(),
        ..page(screen)
    }
}

fn painting(page: Page, size: [u32; 2]) -> BrowserSnapshot {
    BrowserSnapshot::new(size, &render(&page))
}

fn dark(page: Page, size: [u32; 2]) -> BrowserSnapshot {
    beui::html::choose_theme(Some(beui::ThemeKind::Dark));
    let snapshot = painting(page, size);
    beui::html::choose_theme(None);
    snapshot
}
