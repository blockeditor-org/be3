use beui::native::testing::Gui;
use std::time::Duration;

use beui::native::WindowSize;
use beui::native::testing::{Showing, Stepped};
use beui::{Input, PointerButton, native::Context, native::FreetypeFonts, vec2};

use super::*;
use beui::native::build;

fn painting(
    name: &str,
    browser: Option<crate::accounts::BrowserLogin>,
    error: Option<String>,
    select_default: bool,
) {
    painting_sized(name, browser, error, select_default, vec2(1100.0, 720.0));
}

fn painting_sized(
    name: &str,
    browser: Option<crate::accounts::BrowserLogin>,
    error: Option<String>,
    select_default: bool,
    size: beui::Vec2,
) {
    let document = build(move || {
        let view = AppViewStore::new(crate::ui::AppView {
            add_account: crate::ui::AddAccountView {
                open: true,
                pending: browser.is_some(),
                browser,
                error,
                ..Default::default()
            },
            ..Default::default()
        });
        view! {
            <beui::styled::Root>
                <AddAccountDialog
                    view
                    default_server={Some(build_id::DefaultServer {
                        name: "Block".into(), api_url: "https://api.example.com".into(), auth_url: "https://api.example.com/auth".into(), gateway_required: true,
                    })}
                />
            </beui::styled::Root>
        }
    });
    painting_of_sized(name, document, select_default, size);
}

fn painting_of(name: &str, document: beui::native::Document, select_default: bool) {
    painting_of_sized(name, document, select_default, vec2(1100.0, 720.0));
}

fn painting_of_sized(
    name: &str,
    document: beui::native::Document,
    select_default: bool,
    size: beui::Vec2,
) {
    let window = WindowSize { size, scale: 1.0 };
    let context = Context::new(FreetypeFonts::default());
    let mut stepped = Stepped::new(context, Showing(document), window, None);
    for _ in 0..2 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
    if select_default {
        let document = stepped.document();
        let button = document.find_test_id("account.default").unwrap();
        let at = document.node_rect(button).unwrap().center();
        for pressed in [true, false] {
            stepped.frame(vec![
                Input::PointerMoved(at),
                Input::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed,
                },
            ]);
        }
    }
    stepped.advance(Duration::from_secs(1));
    let output = stepped.frame(Vec::new());
    let painted = beui::native::painting::painting(
        output.main(),
        size,
        1.0,
        output.main().window.appearance.clear_color,
        beui::native::painting::Punches::Shown,
    )
    .unwrap();
    let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
    paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
}

mod a_deployed_default_server_asks_for_its_gateway_token;
mod an_expired_session_signs_in_through_the_browser;
mod browser_authorization_always_offers_the_link_and_code;
mod browser_authorization_on_a_narrow_screen;
mod browser_launch_failure_keeps_the_fallback_visible;
mod the_gateway_form_on_a_narrow_screen;
