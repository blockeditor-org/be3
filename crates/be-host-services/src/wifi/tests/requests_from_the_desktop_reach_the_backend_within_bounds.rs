use super::*;

fn connect(ssid: &str, password: Option<&str>) -> WifiRequest {
    WifiRequest::Connect {
        ssid: ssid.to_owned(),
        password: password.map(str::to_owned),
    }
}

fn hidden(ssid: &str, security: WifiSecurity, password: Option<&str>) -> WifiRequest {
    WifiRequest::JoinHidden {
        ssid: ssid.to_owned(),
        security,
        password: password.map(str::to_owned),
    }
}

#[test]
fn requests_from_the_desktop_reach_the_backend_within_bounds() {
    let recorder = Recorder::default();
    let (reports, _) = be_host_base::host::mailbox(|_: WifiStatus| {});
    let wifi = Wifi::with(Rc::new(recorder.clone()), reports);
    let hex = "0123456789abcdef".repeat(4);
    let fine = [
        WifiRequest::SetEnabled(false),
        WifiRequest::Scan,
        WifiRequest::Disconnect,
        connect("Cafe", None),
        connect("Home", Some("correct horse")),
        connect("Home", Some(&hex)),
        connect("Old", Some("abcde")),
        connect("Old", Some("0123456789abcdef0123456789")),
        hidden("Attic", WifiSecurity::Open, None),
        hidden("Attic", WifiSecurity::Password, Some("correct horse")),
        hidden("Attic", WifiSecurity::Wep, Some("abcde")),
    ];
    for request in fine.clone() {
        wifi.request(request);
    }
    assert_eq!(recorder.sent(), fine.to_vec());

    for request in [
        connect("", None),
        connect("Ho\0me", None),
        connect("Home", Some("four")),
        connect("Home", Some(&"x".repeat(64))),
        connect("Home", Some("new\nline password")),
        hidden("", WifiSecurity::Open, None),
        hidden(&"x".repeat(33), WifiSecurity::Open, None),
        hidden("Attic", WifiSecurity::Open, Some("correct horse")),
        hidden("Attic", WifiSecurity::Password, None),
        hidden("Attic", WifiSecurity::Wep, Some("a long passphrase")),
        hidden("Attic", WifiSecurity::Enterprise, Some("correct horse")),
    ] {
        wifi.request(request);
    }
    assert_eq!(
        recorder.sent(),
        Vec::new(),
        "names and passwords no network could have are dropped"
    );
}
