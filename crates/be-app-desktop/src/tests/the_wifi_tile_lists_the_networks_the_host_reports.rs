use be_sdk_beui::beui::icons::{ICON_WIFI, ICON_WIFI_OFF};

use super::*;

#[test]
fn the_wifi_tile_lists_the_networks_the_host_reports() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.status.wifi"),
        "there is no Wi-Fi to show before the host reports a device"
    );

    fixture.wifi(nearby());
    assert!(fixture.test.shown("desktop.status.wifi"));
    assert_eq!(
        fixture.test.label("desktop.status.wifi"),
        ICON_WIFI.as_str(),
        "the bar shows the strength of the network joined"
    );

    fixture.test.click("desktop.status");
    fixture.settle();
    assert!(fixture.test.shown("desktop.quick.wifi"));
    assert!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::wifi)
            .is_empty(),
        "quick settings alone does not look for networks"
    );
    fixture.test.click("desktop.quick.wifi");
    fixture.settle();
    assert!(fixture.test.shown("desktop.wifi.panel"));
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
        vec![WifiRequest::Scan],
        "opening the list looks for networks"
    );
    assert!(fixture.test.shown("desktop.wifi.network.Home.disconnect"));
    assert!(fixture.says("Connected"));
    assert!(fixture.says("Needs a work or school sign-in this desktop cannot set up"));
    assert!(fixture.says("Uses WEP, an old and easily broken security"));
    assert_eq!(
        fixture
            .test
            .accessible_label("desktop.wifi.network.Home")
            .as_deref(),
        Some("Home, signal 82%, secured, Connected"),
        "a network's row is named for what it shows"
    );
    assert_eq!(
        fixture
            .test
            .accessible_label("desktop.wifi.network.Cafe")
            .as_deref(),
        Some("Cafe, signal 48%, open")
    );
    assert!(
        fixture.test.shown("desktop.wifi.network.Home.lock")
            && fixture.test.shown("desktop.wifi.network.Old.lock")
            && !fixture.test.shown("desktop.wifi.network.Cafe.lock"),
        "a protected network is marked with a lock beside its strength"
    );
    fixture.test.snapshot("the_wifi_networks");

    fixture.wifi(WifiStatus {
        link: WifiLink::Connecting("Flat".to_owned()),
        ..nearby()
    });
    assert!(fixture.says("Connecting…"));
    assert!(!fixture.test.shown("desktop.wifi.network.Home.disconnect"));
    let top = |fixture: &Fixture, ssid: &str| {
        fixture
            .test
            .rect_of(&format!("desktop.wifi.network.{ssid}"))
            .min
            .y
    };
    assert!(
        top(&fixture, "Flat") < top(&fixture, "Home"),
        "the network being joined is listed first"
    );
    fixture.wifi(WifiStatus {
        link: WifiLink::Connected("Flat".to_owned()),
        ..nearby()
    });
    assert!(
        top(&fixture, "Flat") < top(&fixture, "Home"),
        "the network joined is listed first"
    );
    assert!(fixture.test.shown("desktop.wifi.network.Flat.disconnect"));

    let switch = fixture.test.rect_of("desktop.wifi.enabled");

    fixture.wifi(WifiStatus {
        enabled: false,
        link: WifiLink::Disconnected,
        networks: Vec::new(),
        ..nearby()
    });
    assert!(fixture.says("Wi-Fi is off."));
    assert_eq!(
        fixture.test.rect_of("desktop.wifi.enabled"),
        switch,
        "the switch stays under the pointer when Wi-Fi goes off"
    );
    assert!(!fixture.test.shown("desktop.wifi.other"));
    fixture.test.snapshot("the_wifi_turned_off");
    assert!(
        fixture
            .test
            .label("desktop.status.wifi")
            .starts_with(ICON_WIFI_OFF.as_str())
    );

    fixture.test.click("desktop.quick.back");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.quick.wifi"),
        "back leads to the tiles"
    );

    fixture.wifi(WifiStatus::default());
    assert!(
        !fixture.test.shown("desktop.status.wifi") && !fixture.test.shown("desktop.quick.wifi"),
        "the icon and the tile go when the Wi-Fi device does"
    );
}
