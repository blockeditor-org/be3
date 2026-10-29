use super::*;
use beui_host::native::build;

#[test]
fn an_expired_session_signs_in_through_the_browser() {
    let document = build(|| {
        let view = AppViewStore::new(crate::ui::AppView {
            reauth: Some(crate::ui::ReauthView {
                name: "Ada".into(),
                busy: false,
                error: None,
            }),
            ..Default::default()
        });
        view! {
            <beui::styled::Root>
                <ReauthDialog view />
            </beui::styled::Root>
        }
    });
    painting_of(
        "an_expired_session_signs_in_through_the_browser",
        document,
        false,
    );
}
