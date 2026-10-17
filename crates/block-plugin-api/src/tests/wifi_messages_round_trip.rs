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
        ],
        failed: Some("Office".to_owned()),
    };
    host_value_round_trips::<Wifi>(status);
    host_value_round_trips::<Wifi>(WifiStatus::default());
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
        WifiRequest::Disconnect,
    ] {
        host_action_round_trips(request);
    }
}
