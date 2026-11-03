use super::*;

const WINDOWS_BACKS: [&str; 3] = [
    "be3:desktop/windows@0.1.0",
    "be3:desktop/programs@0.1.0",
    "be3:desktop/power@0.1.0",
];

const SESSION: [&str; 8] = [
    "be3:desktop/displays@0.1.0",
    "be3:desktop/tray@0.1.0",
    "be3:desktop/media@0.1.0",
    "be3:desktop/wifi@0.1.0",
    "be3:desktop/notifications@0.1.0",
    "be3:desktop/screen-lock@0.1.0",
    "be3:desktop/input@0.1.0",
    "be3:desktop/epaper@0.1.0",
];

#[test]
fn the_desktop_interfaces_are_offered_where_the_host_backs_them() {
    for interface in WINDOWS_BACKS {
        assert!(offers(interface, Platform::Windows, true), "{interface}");
        assert!(
            !offers(interface, Platform::Windows, false),
            "Windows watches its windows only for the desktop: {interface}"
        );
    }
    for interface in SESSION {
        assert!(!offers(interface, Platform::Windows, true), "{interface}");
        assert!(offers(interface, Platform::Linux, true), "{interface}");
        assert!(
            offers(interface, Platform::Linux, false),
            "the session's services are offered on the displays without the desktop: {interface}"
        );
    }
    for interface in ["be3:desktop/windows@0.1.0", "be3:desktop/programs@0.1.0"] {
        assert!(offers(interface, Platform::Linux, true), "{interface}");
        assert!(
            !offers(interface, Platform::Linux, false),
            "only the desktop runs programs in windows of its own: {interface}"
        );
    }
    assert!(offers("be3:desktop/power@0.1.0", Platform::Linux, false));
    for interface in WINDOWS_BACKS.into_iter().chain(SESSION) {
        assert!(!offers(interface, Platform::Other, true), "{interface}");
    }
}
