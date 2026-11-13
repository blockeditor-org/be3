use super::*;

#[test]
fn a_catch_up_forgets_what_the_index_retired_before_the_states_the_server_keeps() {
    let harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 1..17, true);
    assert!(
        index_of(&device, index).index().remembered() > 0,
        "the last compaction's retired segments are remembered while a kept state predates it"
    );
    // The server keeps one state a slot (be-server's `retention`): the earlier ones in it are
    // gone, so a catch-up's watermark passes every compaction before it.
    write_all(&device, index, 17..19, true);
    assert_eq!(
        index_of(&device, index).index().remembered(),
        0,
        "what was retired before every kept state is forgotten"
    );
}
