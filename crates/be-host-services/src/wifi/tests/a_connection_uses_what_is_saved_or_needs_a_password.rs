use super::*;

#[test]
fn a_connection_uses_what_is_saved_or_needs_a_password() {
    let points = [
        point("Home", 40, PSK),
        point("Cafe", 55, 0),
        point("Home", 75, PSK),
        point("Office", 60, EAP),
        AccessPoint {
            flags: 1,
            ..point("Old", 30, 0)
        },
    ];
    let saved = vec![b"Work".to_vec(), b"Home".to_vec()];
    assert_eq!(
        plan("Home", None, &points, &saved),
        Plan::Activate {
            connection: 1,
            point: Some(2),
        },
        "a saved network is connected with what was saved, through its strongest access point"
    );
    assert_eq!(
        plan("Work", None, &points, &saved),
        Plan::Activate {
            connection: 0,
            point: None,
        },
        "a saved network out of sight is still tried"
    );
    assert_eq!(
        plan("Home", Some("a new password".to_owned()), &points, &saved),
        Plan::Add {
            ssid: b"Home".to_vec(),
            key: Key::Psk("a new password".to_owned()),
            hidden: false,
            point: Some(2),
            replaces: vec![1],
        },
        "a new password makes a new connection that replaces the saved one once it works"
    );
    assert_eq!(
        plan("Cafe", None, &points, &saved),
        Plan::Add {
            ssid: b"Cafe".to_vec(),
            key: Key::Open,
            hidden: false,
            point: Some(1),
            replaces: Vec::new(),
        }
    );
    assert_eq!(
        plan("Home", None, &points, &[]),
        Plan::Refuse,
        "a protected network nothing was saved for needs its password"
    );
    assert_eq!(
        plan("Office", Some("password".to_owned()), &points, &saved),
        Plan::Refuse
    );
    assert_eq!(plan("Nowhere", None, &points, &saved), Plan::Refuse);
    assert_eq!(
        plan("Home", Some("short".to_owned()), &points, &saved),
        Plan::Refuse,
        "a WEP key is not a WPA password"
    );
    assert_eq!(
        plan("Old", Some("0123456789".to_owned()), &points, &saved),
        Plan::Add {
            ssid: b"Old".to_vec(),
            key: Key::Wep("0123456789".to_owned()),
            hidden: false,
            point: Some(4),
            replaces: Vec::new(),
        },
        "a WEP network is joined with its key"
    );
    assert_eq!(
        plan("Old", Some("a long passphrase".to_owned()), &points, &saved),
        Plan::Refuse,
        "WEP keys have fixed lengths"
    );
    assert_eq!(
        plan_hidden(
            "Home",
            WifiSecurity::Password,
            Some("correct horse".to_owned()),
            &saved
        ),
        Plan::Add {
            ssid: b"Home".to_vec(),
            key: Key::Psk("correct horse".to_owned()),
            hidden: true,
            point: None,
            replaces: vec![1],
        },
        "a hidden network is named, not found, and replaces what was saved for it"
    );
    assert_eq!(
        plan_hidden("Attic", WifiSecurity::Open, None, &saved),
        Plan::Add {
            ssid: b"Attic".to_vec(),
            key: Key::Open,
            hidden: true,
            point: None,
            replaces: Vec::new(),
        }
    );
    assert_eq!(
        plan_hidden("Attic", WifiSecurity::Wep, Some("abcde".to_owned()), &saved),
        Plan::Add {
            ssid: b"Attic".to_vec(),
            key: Key::Wep("abcde".to_owned()),
            hidden: true,
            point: None,
            replaces: Vec::new(),
        }
    );
    for (security, password) in [
        (WifiSecurity::Password, None),
        (WifiSecurity::Password, Some("short")),
        (WifiSecurity::Wep, Some("not a key")),
        (WifiSecurity::Enterprise, Some("password")),
    ] {
        assert_eq!(
            plan_hidden("Attic", security, password.map(str::to_owned), &saved),
            Plan::Refuse,
            "{security:?} with {password:?}"
        );
    }
}
