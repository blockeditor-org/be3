use std::time::Duration;

use beui::{Context, Event, FreetypeFonts, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};

use super::*;

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

fn painting_of(name: &str, document: beui::Document, select_default: bool) {
    painting_of_sized(name, document, select_default, vec2(1100.0, 720.0));
}

fn painting_of_sized(
    name: &str,
    mut document: beui::Document,
    select_default: bool,
    size: beui::Vec2,
) {
    let context = Context::new(FreetypeFonts::default());
    context.stop_clock();
    {
        let mut frame = |events| {
            context.advance_clock(Duration::from_millis(300));
            context.run(RawInput { events }, |context| {
                document.show(context, Rect::from_min_size(Pos2::ZERO, size))
            })
        };
        let _ = frame(Vec::new());
        let _ = frame(Vec::new());
    }
    if select_default {
        let button = document.find_test_id("account.default").unwrap();
        let at = document.node_rect(button).unwrap().center();
        for pressed in [true, false] {
            context.run(
                RawInput {
                    events: vec![
                        Event::PointerMoved(at),
                        Event::PointerButton {
                            pos: at,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                },
                |context| document.show(context, Rect::from_min_size(Pos2::ZERO, size)),
            );
        }
    }
    context.advance_clock(Duration::from_secs(1));
    let output = context.run(RawInput::default(), |context| {
        document.show(context, Rect::from_min_size(Pos2::ZERO, size))
    });
    let painted = beui::painting::painting(
        &output,
        size,
        1.0,
        output.window.appearance.clear_color,
        beui::painting::Punches::Shown,
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
