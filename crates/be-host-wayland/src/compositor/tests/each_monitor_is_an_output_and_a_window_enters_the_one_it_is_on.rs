use super::*;

fn entered(harness: &Harness) -> Vec<String> {
    let received = &harness.client.received;
    received
        .entered
        .iter()
        .filter_map(|global| {
            received
                .output_names
                .iter()
                .find(|(held, _)| held == global)
                .map(|(_, name)| name.clone())
        })
        .collect()
}

#[test]
fn each_monitor_is_an_output_and_a_window_enters_the_one_it_is_on() {
    let mut harness = Harness::new();
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN.x / 2.0, SCREEN.y));
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness.compositor().set_monitors(monitors(&[left, right]));
    harness.settle();
    let outputs = harness.client.outputs();
    assert_eq!(outputs.len(), 2, "one wl_output per monitor: {outputs:?}");
    harness.client.bind_outputs();
    harness.settle();
    let (_window, id) = harness.open();
    assert!(left.contains_rect(harness.window(id)));
    assert_eq!(
        entered(&harness),
        ["monitor 0"],
        "the window enters the output of the monitor it is on, and only that one"
    );

    let mut plugged = monitors(&[left, right]);
    plugged[0].id = "replaced".to_owned();
    harness.compositor().set_monitors(plugged);
    harness.settle();
    assert_eq!(
        harness.client.outputs().len(),
        2,
        "an unplugged monitor's output goes and the new one's comes"
    );
    assert!(!harness.client.received.removed.is_empty());
    harness.client.bind_outputs();
    harness.settle();
    assert_eq!(
        entered(&harness),
        ["replaced"],
        "the window enters the output that took the old one's place"
    );
}
