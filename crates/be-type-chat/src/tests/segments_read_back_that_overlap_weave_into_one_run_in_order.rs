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
    // Four segments from two devices that closed twice each while apart, one having deleted the
    // third message first, in the time order a merged index keeps.
    let (a1, a2) = (range(0, 5), range(5, 10));
    let b1: Vec<_> = [0, 1, 3, 4].iter().map(|at| message(all[*at])).collect();
    let b2 = range(5, 11);
    assert_eq!(ids(&ChatContent::weave(&[&a1, &b1, &a2, &b2])), all[0..11]);
    assert_eq!(ids(&ChatContent::weave(&[&b1, &a1, &b2, &a2])), all[0..11]);
    // Read in part, as the view reads the newest first and goes back: nothing shows out of order.
    assert_eq!(ids(&ChatContent::weave(&[&b2])), all[5..11]);
    assert_eq!(ids(&ChatContent::weave(&[&a2, &b2])), all[5..11]);
}
