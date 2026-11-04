use super::*;

#[test]
fn wifi_messages_round_trip() {
    let status = WifiStatus {
        available: true,
        enabled: true,
        link: WifiLink::Connected("Home".to_owned()),
        networks: vec![
            WifiNetwork {
                ssid: "Home".to_owned(),
                strength: 82,
                security: WifiSecurity::Password,
                saved: true,
            },
            WifiNetwork {
                ssid: "Cafe".to_owned(),
                strength: 40,
                security: WifiSecurity::Open,
                saved: false,
            },
            WifiNetwork {
                ssid: "Old".to_owned(),
                strength: 30,
                security: WifiSecurity::Wep,
                saved: false,
            },
            WifiNetwork {
                ssid: "Office".to_owned(),
                strength: 20,
                security: WifiSecurity::Enterprise,
                saved: false,
            },
        ],
        failed: Some("Office".to_owned()),
    };
    watched_round_trips::<WifiWatch>(status);
    watched_round_trips::<WifiWatch>(WifiStatus::default());
    for request in [
        WifiRequest::SetEnabled(false),
        WifiRequest::Scan,
        WifiRequest::Connect {
            ssid: "Home".to_owned(),
            password: Some("correct horse".to_owned()),
        },
        WifiRequest::Connect {
            ssid: "Cafe".to_owned(),
            password: None,
        },
        WifiRequest::JoinHidden {
            ssid: "Attic".to_owned(),
            security: WifiSecurity::Wep,
            password: Some("abcde".to_owned()),
        },
        WifiRequest::Disconnect,
    ] {
        round_trips(&request);
    }
}
