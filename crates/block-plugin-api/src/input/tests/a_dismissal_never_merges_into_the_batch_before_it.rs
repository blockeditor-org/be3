use super::*;
use crate::ViewId;

#[test]
fn a_dismissal_never_merges_into_the_batch_before_it() {
    let mut queued = Vec::new();
    InputBatch::queue(&mut queued, input(Input::Zoom(2.0)));
    let dismissing = InputBatch {
        dismissed: vec![ViewId(4)],
        ..input(Input::Zoom(1.5))
    };
    InputBatch::queue(&mut queued, dismissing.clone());
    InputBatch::queue(&mut queued, input(Input::Zoom(2.0)));

    assert_eq!(
        queued,
        vec![
            input(Input::Zoom(2.0)),
            InputBatch {
                events: vec![Input::Zoom(3.0).into()],
                ..dismissing
            },
        ]
    );
}
