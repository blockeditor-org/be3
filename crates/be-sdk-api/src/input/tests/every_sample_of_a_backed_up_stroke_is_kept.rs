use super::*;

#[test]
fn every_sample_of_a_backed_up_stroke_is_kept() {
    let mut queued = Vec::new();
    InputBatch::queue(&mut queued, batch(vec![pen(1.0, 10)], true));
    InputBatch::queue(&mut queued, batch(vec![pen(2.0, 20)], true));
    InputBatch::queue(&mut queued, batch(vec![pen(3.0, 30), pen(4.0, 40)], true));

    assert_eq!(queued.len(), 1);
    assert_eq!(
        queued.first().cloned(),
        Some(batch(
            vec![pen(1.0, 10), pen(2.0, 20), pen(3.0, 30), pen(4.0, 40)],
            true
        ))
    );
}
