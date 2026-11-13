use super::*;

#[test]
fn a_search_index_is_compacted_by_the_device_that_writes_it() {
    let harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 0..15, false);
    let content = index_of(&device, index);
    // Tiered by fours: fifteen documents are three segments of four and three of one.
    assert_eq!(
        content.index().segment_count(),
        6,
        "the batches were compacted"
    );
    for n in 0..15 {
        assert_eq!(finds(&device, index, n), 1, "{} is found", word(n));
    }
}
