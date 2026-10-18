use super::*;

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
            <Frame fill=ColorRole::Background>
                <ReauthDialog view />
            </Frame>
        }
    });
    painting_of(
        "an_expired_session_signs_in_through_the_browser",
        document,
        false,
    );
}
