use super::*;

use be_sdk_api::{HostWindow, HostWindowId, WindowKind, WindowOrigin, WindowsWatch};

#[test]
fn the_program_windows_reach_only_the_shell() {
    crate::run_as_desktop();
    let mut instances = placed();
    let windows = vec![HostWindow {
        id: HostWindowId(3),
        kind: WindowKind::Window,
        parent: None,
        modal: false,
        program_id: "foot".to_owned(),
        origin: WindowOrigin::default(),
    }];
    instances.next_screens(PASS);

    assert!(watch::<WindowsWatch>(&mut instances));
    assert!(
        !publish::<WindowsWatch>(&mut instances, &windows),
        "a session opened inside the desktop does not see the program windows"
    );
    assert!(watched_sent::<WindowsWatch>(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.set_shell(Some(INSTANCE)));
    assert_eq!(
        watched_sent::<WindowsWatch>(&instances.next_screens(PASS).opened),
        vec![windows],
        "the shell sees them"
    );
}
