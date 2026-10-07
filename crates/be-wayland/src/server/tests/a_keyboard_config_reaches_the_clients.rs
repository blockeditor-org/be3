use super::*;

use crate::state::KeyboardConfig;

#[test]
fn a_keyboard_config_reaches_the_clients() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _keyboard = client.keyboard();
    client.exchange(&mut server);
    let keymaps = client.received.keymaps;
    assert_eq!(client.received.repeat, Some((25, 600)));

    let german = KeyboardConfig {
        layout: "de".to_owned(),
        variant: String::new(),
        options: String::new(),
        repeat_delay: 300,
        repeat_rate: 40,
    };
    assert!(server.state.set_keyboard(&german));
    client.exchange(&mut server);
    assert_eq!(client.received.repeat, Some((40, 300)));
    assert_eq!(client.received.keymaps, keymaps + 1);

    let unknown = KeyboardConfig {
        layout: "no-such-layout".to_owned(),
        ..german
    };
    assert!(!server.state.set_keyboard(&unknown));
}
