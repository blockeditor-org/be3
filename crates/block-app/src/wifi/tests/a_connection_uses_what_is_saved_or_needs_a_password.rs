use super::*;

#[test]
fn a_connection_uses_what_is_saved_or_needs_a_password() {
    let points = [
        point("Home", 40, PSK),
        point("Cafe", 55, 0),
        point("Home", 75, PSK),
        point("Office", 60, EAP),
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
            key_management: Some("wpa-psk"),
            password: Some("a new password".to_owned()),
            point: Some(2),
            replaces: vec![1],
        },
        "a new password makes a new connection that replaces the saved one once it works"
    );
    assert_eq!(
        plan("Cafe", None, &points, &saved),
        Plan::Add {
            ssid: b"Cafe".to_vec(),
            key_management: None,
            password: None,
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
}
