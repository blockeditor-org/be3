use super::*;

use block_plugin_api::{HostValue, HostWindow, HostWindowId, HostWindows, Size};

#[test]
fn a_shell_only_host_value_reaches_only_the_shell() {
    let mut instances = placed();
    let windows = vec![HostWindow {
        id: HostWindowId(1),
        title: "Terminal".to_owned(),
        app_id: "foot".to_owned(),
        parent: None,
        size: Size {
            width: 640.0,
            height: 480.0,
        },
        fullscreen: None,
        responding: true,
        focused: false,
    }];
    instances.next_screens(PASS);

    assert!(watch::<HostWindows>(&mut instances));
    assert!(
        !publish::<HostWindows>(&mut instances, &windows),
        "only the shell may see the windows"
    );
    assert!(host_values_sent::<HostWindows>(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.set_shell(Some(INSTANCE)));
    assert!(instances.watches_host_value(HostWindows::KEY));
    assert_eq!(
        host_values_sent::<HostWindows>(&instances.next_screens(PASS).opened),
        vec![windows.clone()],
        "an instance that becomes the shell is told what it was waiting for"
    );

    instances.set_shell(None);
    assert!(
        !publish::<HostWindows>(&mut instances, &Vec::new()),
        "nor does an instance that is no longer the shell"
    );
    assert!(host_values_sent::<HostWindows>(&instances.next_screens(PASS).opened).is_empty());
}
