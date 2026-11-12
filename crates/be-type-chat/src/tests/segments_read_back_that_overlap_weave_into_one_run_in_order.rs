use super::*;

#[test]
fn segments_read_back_that_overlap_weave_into_one_run_in_order() {
    let all = long(60).messages();
    let message = |id: ObjectId| (id, Message::new(ADA, 0, ""));
    let range = |from: usize, to: usize| -> Vec<(ObjectId, Message)> {
        all[from..to].iter().copied().map(message).collect()
    };
    let ids = |run: &[(ObjectId, Message)]| run.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    // One segment closed a message further than the other, and one that starts later comes
    // first in the index.
    let (a, b) = (range(1, 50), range(2, 51));
    assert_eq!(ids(&ChatContent::weave(&[&a, &b])), all[1..51]);
    assert_eq!(ids(&ChatContent::weave(&[&b, &a])), all[1..51]);
    // Segments that do not overlap follow one another.
    let (early, late) = (range(0, 10), range(10, 20));
    assert_eq!(ids(&ChatContent::weave(&[&early, &late])), all[0..20]);
    assert_eq!(ids(&ChatContent::weave(&[&early, &early])), all[0..10]);
}
