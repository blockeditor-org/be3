use super::*;

use be_sdk_api::{
    HostWindow, HostWindowId, WindowAction, WindowKind, WindowOrigin, WindowsWatch, requested,
};

#[test]
fn the_program_windows_reach_only_the_shell() {
    let mut instances = placed();
    let windows = vec![HostWindow {
        id: HostWindowId(3),
        kind: WindowKind::Window,
        shown: true,
        title: "foot".to_owned(),
        program_id: "foot".to_owned(),
        name: "Foot".to_owned(),
        parent: None,
        modal: false,
        size: Vec2::new(320.0, 200.0),
        fullscreen: None,
        responding: true,
        focused: true,
        icon: None,
        raised: 0,
        origin: WindowOrigin::default(),
        note: None,
    }];
    instances.next_screens(PASS);

    assert!(watch::<WindowsWatch>(&mut instances));
    assert!(
        !publish::<WindowsWatch>(&mut instances, &windows),
        "a session opened inside the desktop does not see the program windows"
    );
    assert!(watched_sent::<WindowsWatch>(&instances.next_screens(PASS).opened).is_empty());
    let close = WindowAction::Close(HostWindowId(3));
    assert!(
        !host_call::<WindowsWatch, _>(&mut instances, "request", &close),
        "nor may it act on them"
    );

    assert!(instances.set_shell(Some(INSTANCE)));
    assert_eq!(
        watched_sent::<WindowsWatch>(&instances.next_screens(PASS).opened),
        vec![windows],
        "the shell sees them"
    );
    assert!(host_call::<WindowsWatch, _>(
        &mut instances,
        "request",
        &close
    ));
    let mut taken = Vec::new();
    instances.take_calls(&mut |called| match requested::window(called) {
        Some(action) => {
            taken.push(action);
            true
        }
        None => false,
    });
    assert_eq!(taken, vec![close], "and acts on them");
}
