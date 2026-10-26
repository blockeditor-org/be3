use block_editor_beui::beui::icons::{ICON_WIFI, ICON_WIFI_OFF};

use super::*;

#[test]
fn the_wifi_in_the_bar_lists_the_networks_the_host_reports() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.wifi"),
        "there is no Wi-Fi to show before the host reports a device"
    );

    fixture.wifi(nearby());
    assert!(fixture.test.shown("desktop.wifi"));
    assert_eq!(
        fixture.test.label("desktop.wifi"),
        ICON_WIFI.as_str(),
        "the bar shows the strength of the network joined"
    );
    fixture.test.snapshot("the_wifi_in_the_bar");

    fixture.test.click("desktop.wifi");
    fixture.settle();
    assert!(fixture.test.shown("desktop.wifi.panel"));
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::wifi),
        vec![WifiRequest::Scan],
        "opening the list looks for networks"
    );
    assert!(fixture.test.shown("desktop.wifi.network.Home.disconnect"));
    assert!(fixture.says("Connected"));
    assert!(fixture.says("Needs a sign-in this desktop cannot do"));
    fixture.test.snapshot("the_wifi_popup");

    fixture.wifi(WifiStatus {
        link: WifiLink::Connecting("Cafe".to_owned()),
        ..nearby()
    });
    assert!(fixture.says("Connecting…"));
    assert!(!fixture.test.shown("desktop.wifi.network.Home.disconnect"));

    fixture.wifi(WifiStatus {
        enabled: false,
        link: WifiLink::Disconnected,
        networks: Vec::new(),
        ..nearby()
    });
    assert!(fixture.says("Wi-Fi is off."));
    assert!(
        fixture
            .test
            .label("desktop.wifi")
            .starts_with(ICON_WIFI_OFF.as_str())
    );

    fixture.wifi(WifiStatus::default());
    assert!(
        !fixture.test.shown("desktop.wifi"),
        "the item goes when the Wi-Fi device does"
    );
}
