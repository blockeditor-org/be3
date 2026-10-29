use super::*;
use crate::ViewId;

#[test]
fn scrolls_over_different_anchored_views_stay_apart() {
    let scroll = |anchored| TimedInput {
        anchored,
        ..Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 10.0))).into()
    };
    let mut queued = Vec::new();
    InputBatch::queue(&mut queued, input(scroll(None)));
    InputBatch::queue(&mut queued, input(scroll(Some(ViewId(1)))));
    InputBatch::queue(&mut queued, input(scroll(Some(ViewId(1)))));

    let doubled = TimedInput {
        anchored: Some(ViewId(1)),
        ..Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 20.0))).into()
    };
    assert_eq!(queued, vec![batch(vec![scroll(None), doubled], false)]);
}
