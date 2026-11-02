use super::*;

#[test]
fn the_primary_selection_is_advertised_whether_or_not_it_is_switched_on() {
    let mut server = server();
    for on in [false, true, false] {
        server.state.set_primary_selection(on);
        let client = TestClient::connect(&mut server);
        assert_eq!(
            advertised(&client, "zwp_primary_selection_device_manager_v1"),
            1,
            "advertised with middle-click paste {}",
            if on { "on" } else { "off" }
        );
        assert_eq!(advertised(&client, "ext_data_control_manager_v1"), 1);
        assert_eq!(advertised(&client, "zwlr_data_control_manager_v1"), 1);
        assert_eq!(advertised(&client, "wl_data_device_manager"), 1);
    }
}
