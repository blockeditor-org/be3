use super::*;

#[test]
fn a_title_reaches_the_ui() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    window.toplevel.set_title("Terminal".to_owned());
    client.exchange(&mut server);

    assert!(
        server
            .state
            .take_events()
            .contains(&ServerEvent::Titled(id, "Terminal".to_owned()))
    );
    assert_eq!(server.state.title(id).as_deref(), Some("Terminal"));
}
