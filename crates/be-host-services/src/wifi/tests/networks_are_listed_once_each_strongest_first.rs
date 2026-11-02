use super::*;

#[test]
fn networks_are_listed_once_each_strongest_first() {
    let points = [
        point("Home", 40, PSK),
        point("Cafe", 55, 0),
        point("", 90, 0),
        point("Home", 75, PSK),
        point("Office", 60, EAP | PSK),
        point("Flat", 20, SAE),
        AccessPoint {
            ssid: b"Old".to_vec(),
            strength: 10,
            flags: 1,
            wpa: 0,
            rsn: 0,
        },
    ];
    let saved = vec![b"Home".to_vec()];
    assert_eq!(
        networks(&points, &saved),
        vec![
            WifiNetwork {
                ssid: "Home".to_owned(),
                strength: 75,
                security: WifiSecurity::Password,
                saved: true,
            },
            WifiNetwork {
                ssid: "Office".to_owned(),
                strength: 60,
                security: WifiSecurity::Enterprise,
                saved: false,
            },
            WifiNetwork {
                ssid: "Cafe".to_owned(),
                strength: 55,
                security: WifiSecurity::Open,
                saved: false,
            },
            WifiNetwork {
                ssid: "Flat".to_owned(),
                strength: 20,
                security: WifiSecurity::Password,
                saved: false,
            },
            WifiNetwork {
                ssid: "Old".to_owned(),
                strength: 10,
                security: WifiSecurity::Wep,
                saved: false,
            },
        ],
        "a hidden network is left out, enterprise and WEP networks are told apart, and a name seen twice is listed at its strongest"
    );
    let password = || Some("correct horse".to_owned());
    assert_eq!(
        point("Home", 40, PSK | SAE).key(password()),
        Some(Key::Psk("correct horse".to_owned()))
    );
    assert_eq!(
        point("Flat", 40, SAE).key(password()),
        Some(Key::Sae("correct horse".to_owned()))
    );
    assert_eq!(point("Cafe", 40, 0).key(None), Some(Key::Open));
    assert_eq!(point("Cafe", 40, 0x800).key(None), Some(Key::Owe));
    assert_eq!(point("Office", 40, EAP).key(password()), None);

    let mut points = [point("", 90, PSK), point("", 30, 0)];
    name_hidden(&mut points, Some(0), Some(b"Attic"));
    assert_eq!(
        networks(&points, &[b"Attic".to_vec()]),
        vec![WifiNetwork {
            ssid: "Attic".to_owned(),
            strength: 90,
            security: WifiSecurity::Password,
            saved: true,
        }],
        "the hidden network joined is listed under the name it was joined by"
    );
    assert_eq!(
        link(ACTIVATED, Some("Home".to_owned())),
        WifiLink::Connected("Home".to_owned())
    );
    assert_eq!(
        link(70, Some("Home".to_owned())),
        WifiLink::Connecting("Home".to_owned())
    );
    assert_eq!(
        link(DISCONNECTED, Some("Home".to_owned())),
        WifiLink::Disconnected
    );
}
