use super::*;

/// An ask for a host panel made while the keyboard is in its placer's own layer leaves it there
/// until the root has drawn the panel, rather than taking it out of the placer as for a child
/// whose app is yet to start, and then moves it into the panel's layer.
#[test]
fn a_keyboard_ask_for_a_host_panel_waits_in_its_placers_layer_until_it_is_drawn() {
    let mut children = placing(Vec::new());
    children.panels.push(panel(false));
    ask(&mut children, 9, 1);
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);

    children.panels = vec![panel(true)];
    assert_eq!(
        keyboard(&mut children, Some(PLACER), &[]),
        Some(KeyboardMove::Into(LayerId(300)))
    );
}
