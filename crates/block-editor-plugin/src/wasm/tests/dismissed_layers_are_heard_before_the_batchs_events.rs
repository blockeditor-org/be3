use block_plugin_api::{Input, InputBatch};

use super::*;

#[test]
fn dismissed_layers_are_heard_before_the_batchs_events() {
    let mut rig = Rig::new(Some(space()));
    rig.open(vec![layer(
        1,
        0,
        Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.0)),
    )]);
    rig.draw();
    let moved = |anchored| TimedInput {
        input: Input::PointerMoved(pos2(30.0, 40.0)),
        time_micros: None,
        over: None,
        anchored,
    };
    rig.screens.deliver(Delivery::Input(InputBatch {
        screen: SCREEN,
        events: vec![moved(Some(ViewId(2))), moved(None)],
        every_sample: false,
        dismissed: vec![ViewId(1), ViewId(7)],
    }));
    assert_eq!(
        rig.shared.seen.borrow().heard,
        [
            "dismissed [ViewId(1), ViewId(7)]",
            "input on Some(ViewId(2))",
            "input on None",
        ]
    );
}
