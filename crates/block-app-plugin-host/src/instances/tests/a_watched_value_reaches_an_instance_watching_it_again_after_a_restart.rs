use super::*;

use block_plugin_api::{DisplayMode, DisplaysWatch, HostDisplay, Rotation, Watched};

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
        rect: beui::Rect::ZERO,
        scale: 1.0,
    }
}

#[test]
fn a_watched_value_reaches_an_instance_watching_it_again_after_a_restart() {
    let mut instances = placed();
    instances.next_screens(PASS);

    let first = vec![monitor(2560)];
    assert!(
        !publish::<DisplaysWatch>(&mut instances, &first),
        "nobody is watching yet"
    );
    assert!(watched_sent::<DisplaysWatch>(&instances.next_screens(PASS).opened).is_empty());

    assert!(watch::<DisplaysWatch>(&mut instances));
    assert!(instances.watches_published(DisplaysWatch::OPEN));
    assert_eq!(
        watched_sent::<DisplaysWatch>(&instances.next_screens(PASS).opened),
        vec![first.clone()]
    );
    assert!(
        watched_sent::<DisplaysWatch>(&instances.next_screens(PASS).opened).is_empty(),
        "a value is told once"
    );

    let second = vec![monitor(1920)];
    assert!(publish::<DisplaysWatch>(&mut instances, &second));
    assert_eq!(
        watched_sent::<DisplaysWatch>(&instances.next_screens(PASS).opened),
        vec![second.clone()]
    );

    instances.reopen();
    assert_eq!(
        watched_sent::<DisplaysWatch>(&instances.next_screens(PASS).opened),
        vec![second.clone()],
        "a restarted plugin is told again"
    );
}
