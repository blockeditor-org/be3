use super::*;

#[test]
fn an_offline_catch_up_forgets_nothing() {
    let mut harness = Harness::start();
    harness.connect();
    let device = Device::online(&harness);
    let index = indexed(&device);
    write_all(&device, index, 1..5, true);
    let remembered = index_of(&device, index).index().remembered();
    assert!(remembered > 0, "the compaction retired segments");

    harness.stop_server();
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        assert_eq!(
            crate::search::write(peer.as_ref(), index, one(&word(5), 6, Some(50))).await,
            Ok(true),
            "the batch is written offline"
        );
    });
    assert_eq!(
        index_of(&device, index).index().remembered(),
        remembered,
        "offline, nothing is forgotten"
    );
}
