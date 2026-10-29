use super::*;

fn connect(ssid: &str, password: Option<&str>) -> WifiRequest {
    WifiRequest::Connect {
        ssid: ssid.to_owned(),
        password: password.map(str::to_owned),
    }
}

#[test]
fn requests_from_the_desktop_reach_the_backend_within_bounds() {
    let recorder = Recorder::default();
    let (reports, reported) = std::sync::mpsc::channel();
    let mut wifi = Wifi::with(Rc::new(recorder.clone()), reported);
    let hex = "0123456789abcdef".repeat(4);
    let fine = [
        WifiRequest::SetEnabled(false),
        WifiRequest::Scan,
        WifiRequest::Disconnect,
        connect("Cafe", None),
        connect("Home", Some("correct horse")),
        connect("Home", Some(&hex)),
    ];
    for request in fine.clone() {
        wifi.request(request);
    }
    assert_eq!(recorder.sent(), fine.to_vec());

    for request in [
        connect("", None),
        connect("Ho\0me", None),
        connect("Home", Some("short")),
        connect("Home", Some(&"x".repeat(64))),
        connect("Home", Some("new\nline password")),
    ] {
        wifi.request(request);
    }
    assert_eq!(
        recorder.sent(),
        Vec::new(),
        "names and passwords no network could have are dropped"
    );

    assert_eq!(
        wifi.frame(),
        None,
        "nothing is reported before the backend speaks"
    );
    let status = WifiStatus {
        available: true,
        enabled: true,
        ..WifiStatus::default()
    };
    reports.send(WifiStatus::default()).unwrap();
    reports.send(status.clone()).unwrap();
    assert_eq!(
        wifi.frame(),
        Some(status.clone()),
        "the latest report is the one sent"
    );
    reports.send(status).unwrap();
    assert_eq!(
        wifi.frame(),
        None,
        "a report that changed nothing is not sent again"
    );
}
