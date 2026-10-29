use block_plugin_api::{Input, InputBatch};

use super::*;

#[test]
fn dismissed_layers_are_heard_before_the_batchs_events() {
    let mut rig = Rig::new(Some(space()));
    rig.open(vec![layer(
        1,
        LayerParent::Region,
        Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.0)),
    )]);
    rig.draw();
    let moved = |layer| TimedInput {
        input: Input::PointerMoved(pos2(30.0, 40.0)),
        time_micros: None,
        over: None,
        layer,
    };
    rig.screens.deliver(Delivery::Input(InputBatch {
        screen: SCREEN,
        events: vec![moved(Some(LayerId(2))), moved(None)],
        every_sample: false,
        dismissed: vec![LayerId(1), LayerId(7)],
    }));
    assert_eq!(
        rig.shared.seen.borrow().heard,
        [
            "dismissed [LayerId(1), LayerId(7)]",
            "input on Some(LayerId(2))",
            "input on None",
        ]
    );
}
