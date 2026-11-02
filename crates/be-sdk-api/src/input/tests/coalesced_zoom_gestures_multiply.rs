use super::*;

#[test]
fn coalesced_zoom_gestures_multiply() {
    let mut queued = Vec::new();
    InputBatch::queue(&mut queued, input(Input::Zoom(2.0)));
    InputBatch::queue(&mut queued, input(Input::Zoom(1.5)));

    assert_eq!(queued.len(), 1);
    assert_eq!(queued.first().cloned(), Some(input(Input::Zoom(3.0))));
}
