use super::*;

#[test]
fn the_wifi_popup_asks_the_host_to_connect() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.wifi(nearby());
    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.click("desktop.quick.wifi");
    fixture.settle();
    fixture.test.take_host_calls(block_ui_test::requested::wifi);

    fixture.test.click("desktop.wifi.network.Cafe");
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::wifi),
        vec![WifiRequest::Connect {
            ssid: "Cafe".to_owned(),
            password: None,
        }],
        "an open network is joined at once"
    );

    fixture.test.click("desktop.wifi.network.Office");
    fixture.settle();
    assert!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::wifi)
            .is_empty(),
        "a network this desktop cannot sign in to is not tried"
    );

    fixture.test.click("desktop.wifi.network.Flat");
    fixture.settle();
    assert!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::wifi)
            .is_empty(),
        "a protected network asks for its password first"
    );
    assert!(fixture.test.shown("desktop.wifi.network.Flat.password"));
    fixture.test.type_text("hunter2");
    fixture.settle();
    fixture.test.click("desktop.wifi.network.Flat.connect");
    fixture.settle();
    assert!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::wifi)
            .is_empty(),
        "a password too short for any network is not sent"
    );
    fixture.test.click("desktop.wifi.network.Flat.password");
    fixture.settle();
    fixture.test.type_text("2");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::wifi),
        vec![WifiRequest::Connect {
            ssid: "Flat".to_owned(),
            password: Some("hunter22".to_owned()),
        }]
    );
    assert!(!fixture.test.shown("desktop.wifi.network.Flat.password"));

    fixture.wifi(WifiStatus {
        failed: Some("Flat".to_owned()),
        ..nearby()
    });
    assert!(
        fixture.test.shown("desktop.wifi.network.Flat.password"),
        "a password that did not work is asked for again"
    );
    assert!(fixture.says("Could not connect. Check the password."));
    fixture.test.snapshot("a_wifi_password_asked_again");

    fixture.test.click("desktop.wifi.network.Home.disconnect");
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::wifi),
        vec![WifiRequest::Disconnect]
    );

    fixture.test.click("desktop.wifi.enabled");
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::wifi),
        vec![WifiRequest::SetEnabled(false)]
    );
}
