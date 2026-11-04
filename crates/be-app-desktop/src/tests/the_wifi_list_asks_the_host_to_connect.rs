use super::*;

#[test]
fn the_wifi_list_asks_the_host_to_connect() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.wifi(nearby());
    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.click("desktop.quick.wifi");
    fixture.settle();
    fixture.test.take_host_calls(be_sdk_test::requested::wifi);

    fixture.test.click("desktop.wifi.network.Cafe");
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
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
            .take_host_calls(be_sdk_test::requested::wifi)
            .is_empty(),
        "a network this desktop cannot sign in to is not tried"
    );

    fixture.test.click("desktop.wifi.network.Old");
    fixture.settle();
    assert!(fixture.test.shown("desktop.wifi.network.Old.password"));
    assert_eq!(
        fixture.test.label("desktop.wifi.network.Old.password"),
        "Key",
        "a WEP network asks for its key"
    );
    fixture.test.type_text("abcde");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
        vec![WifiRequest::Connect {
            ssid: "Old".to_owned(),
            password: Some("abcde".to_owned()),
        }],
        "the key is typed into the field that opened, without clicking it first"
    );

    fixture.test.click("desktop.wifi.network.Flat");
    fixture.settle();
    assert!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::wifi)
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
            .take_host_calls(be_sdk_test::requested::wifi)
            .is_empty(),
        "a password too short for any network is not sent"
    );
    fixture.test.click("desktop.wifi.network.Flat.password");
    fixture.settle();
    fixture.test.type_text("2");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
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
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
        vec![WifiRequest::Disconnect]
    );

    fixture.test.click("desktop.wifi.other");
    fixture.settle();
    assert!(fixture.test.shown("desktop.wifi.hidden"));
    fixture.test.type_text("Attic");
    let security = fixture.test.rect_of("desktop.wifi.hidden.security");
    fixture
        .test
        .click(Pos2::new(security.min.x + 8.0, security.center().y));
    fixture.test.key(Key::ArrowRight, Modifiers::NONE);
    fixture.settle();
    fixture.test.click("desktop.wifi.hidden.password");
    fixture.settle();
    fixture.test.type_text("abcde");
    fixture.settle();
    fixture.test.click("desktop.wifi.hidden.connect");
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
        vec![WifiRequest::JoinHidden {
            ssid: "Attic".to_owned(),
            security: WifiSecurity::Wep,
            password: Some("abcde".to_owned()),
        }],
        "a hidden network is joined by its name, security and key"
    );
    fixture.wifi(WifiStatus {
        link: WifiLink::Disconnected,
        failed: Some("Attic".to_owned()),
        ..nearby()
    });
    assert!(fixture.says("Could not connect to Attic"));
    fixture.test.snapshot("a_hidden_wifi_network");
    fixture.wifi(WifiStatus {
        link: WifiLink::Connected("Attic".to_owned()),
        ..nearby()
    });
    assert!(
        !fixture.test.shown("desktop.wifi.hidden"),
        "the form closes once the hidden network is joined"
    );

    fixture.test.click("desktop.wifi.enabled");
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::wifi),
        vec![WifiRequest::SetEnabled(false)]
    );
}
