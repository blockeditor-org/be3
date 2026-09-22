use super::*;

#[test]
fn a_plugin_that_draws_nothing_is_granted_no_surface() {
    let mut session = session();
    session.start(0);
    let hello = Message::Hello(Hello {
        version: PROTOCOL_VERSION,
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1".into(),
        },
        surface: SurfaceSupport::None,
    });
    session.receive_frame(&encode_frame(&hello).unwrap(), 1);
    assert_eq!(session.granted_surface(), None);
    assert!(matches!(
        session.next_outbound(),
        Some(Message::HelloAccepted(HelloAccepted { surface: None, .. }))
    ));
}
