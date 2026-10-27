use super::*;

#[test]
fn windows_offers_the_desktop_interfaces_it_backs_and_no_others() {
    for offered in [
        "be3:desktop/windows@0.1.0",
        "be3:desktop/programs@0.1.0",
        "be3:desktop/power@0.1.0",
    ] {
        assert!(windows_offers(offered), "{offered}");
    }
    for unsupported in [
        "be3:desktop/displays@0.1.0",
        "be3:desktop/tray@0.1.0",
        "be3:desktop/media@0.1.0",
        "be3:desktop/wifi@0.1.0",
        "be3:desktop/notifications@0.1.0",
        "be3:desktop/screen-lock@0.1.0",
        "be3:desktop/input@0.1.0",
        "be3:desktop/epaper@0.1.0",
    ] {
        assert!(!windows_offers(unsupported), "{unsupported}");
    }
}
