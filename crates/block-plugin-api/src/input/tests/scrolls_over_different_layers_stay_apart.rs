use super::*;
use crate::LayerId;

#[test]
fn scrolls_over_different_layers_stay_apart() {
    let scroll = |layer| TimedInput {
        layer,
        ..Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 10.0))).into()
    };
    let mut queued = Vec::new();
    InputBatch::queue(&mut queued, input(scroll(None)));
    InputBatch::queue(&mut queued, input(scroll(Some(LayerId(1)))));
    InputBatch::queue(&mut queued, input(scroll(Some(LayerId(1)))));

    let doubled = TimedInput {
        layer: Some(LayerId(1)),
        ..Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 20.0))).into()
    };
    assert_eq!(queued, vec![batch(vec![scroll(None), doubled], false)]);
}
