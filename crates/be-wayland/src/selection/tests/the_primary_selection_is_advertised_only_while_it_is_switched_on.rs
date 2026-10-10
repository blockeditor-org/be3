use super::*;

#[test]
fn the_primary_selection_is_advertised_only_while_it_is_switched_on() {
    let mut server = server();
    let before = TestClient::connect(&mut server);
    assert_eq!(
        advertised(&before, "zwp_primary_selection_device_manager_v1"),
        0
    );
    assert_eq!(advertised(&before, "ext_data_control_manager_v1"), 1);
    assert_eq!(advertised(&before, "zwlr_data_control_manager_v1"), 1);
    assert_eq!(advertised(&before, "wl_data_device_manager"), 1);

    server.state.set_primary_selection(true);
    let after = TestClient::connect(&mut server);
    assert_eq!(
        advertised(&after, "zwp_primary_selection_device_manager_v1"),
        1
    );
    assert_eq!(advertised(&after, "ext_data_control_manager_v1"), 1);

    server.state.set_primary_selection(false);
    let off = TestClient::connect(&mut server);
    assert_eq!(
        advertised(&off, "zwp_primary_selection_device_manager_v1"),
        0
    );
}
