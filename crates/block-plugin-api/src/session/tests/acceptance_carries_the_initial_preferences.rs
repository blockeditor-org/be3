use super::*;

#[test]
fn acceptance_carries_the_initial_preferences() {
    let preferences = crate::Preferences {
        theme: crate::ThemeKind::EPaper,
        motion: crate::Motion::Instant,
    };
    let mut session = HostSession::new(
        "BE3",
        Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
        }),
        preferences,
    );
    session.start(0);
    session.receive_frame(&encode_frame(&hello()).unwrap(), 1);
    let Some(Message::HelloAccepted(accepted)) = session.next_outbound() else {
        panic!("expected acceptance");
    };
    assert_eq!(accepted.preferences, preferences);
}
