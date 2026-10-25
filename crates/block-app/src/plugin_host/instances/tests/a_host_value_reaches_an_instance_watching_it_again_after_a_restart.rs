use super::*;

use block_plugin_api::{DisplayMode, Displays, HostDisplay, HostValue, Rotation};

fn monitor(width: u32) -> HostDisplay {
    let mode = DisplayMode {
        width,
        height: 1440,
        refresh_millihertz: 239_970,
    };
    HostDisplay {
        id: "DEL|DELL AW2524H|7XQ2B34".to_owned(),
        name: "DELL AW2524H".to_owned(),
        connector: "DP-1".to_owned(),
        modes: vec![mode],
        default: mode,
        current: mode,
        rotation: Rotation::Normal,
    }
}

#[test]
fn a_host_value_reaches_an_instance_watching_it_again_after_a_restart() {
    let mut instances = placed();
    instances.next_screens(PASS);

    let first = vec![monitor(2560)];
    assert!(
        !publish::<Displays>(&mut instances, &first),
        "nobody is watching yet"
    );
    assert!(host_values_sent::<Displays>(&instances.next_screens(PASS).opened).is_empty());

    assert!(watch::<Displays>(&mut instances));
    assert!(instances.watches_host_value(Displays::KEY));
    assert_eq!(
        host_values_sent::<Displays>(&instances.next_screens(PASS).opened),
        vec![first.clone()]
    );
    assert!(
        host_values_sent::<Displays>(&instances.next_screens(PASS).opened).is_empty(),
        "a value is told once"
    );

    let second = vec![monitor(1920)];
    assert!(publish::<Displays>(&mut instances, &second));
    assert_eq!(
        host_values_sent::<Displays>(&instances.next_screens(PASS).opened),
        vec![second.clone()]
    );

    instances.reopen();
    assert_eq!(
        host_values_sent::<Displays>(&instances.next_screens(PASS).opened),
        vec![second.clone()],
        "a restarted plugin is told again"
    );

    assert!(
        !watch_key(&mut instances, "nothing the host keeps"),
        "a value the host does not keep cannot be watched"
    );
}
