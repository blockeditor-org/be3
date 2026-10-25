use super::*;

#[test]
fn input_merged_while_queued_keeps_its_order() {
    let mut queued = Vec::new();
    let keys: Vec<_> = crate::Key::ALL
        .iter()
        .copied()
        .cycle()
        .take(512)
        .collect();
    for key in &keys {
        InputBatch::queue(&mut queued, input(Input::Key(crate::Keystroke {
                key: Some(*key),
                pressed: true,
                ..crate::Keystroke::default()
            })));
    }
    assert_eq!(queued.len(), 1);
    let Some(batch) = queued.first().cloned() else {
        panic!()
    };
    let merged: Vec<_> = batch
        .events
        .iter()
        .map(|event| match &event.input {
            Input::Key(stroke) => stroke.key.unwrap(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(merged, keys);
}
