use super::*;

/// A replica of a type whose content folds its log reads the log on, a page at a time, past
/// the first page and past a gap left by an entry heard ahead of what it read, so what it
/// describes folds every entry, in order.
#[test]
fn a_log_longer_than_a_page_is_folded_whole() {
    let mut sim = Sim::<Logbook>::new(1, logbook_module);
    let item = |seq: u64| be_sdk_type_api::LogItem {
        seq,
        id: Uuid::from_u64_pair(1, seq),
        author: Uuid::from_u128(1),
        arrival: seq as i64,
        payload: Some(seq.to_string().into_bytes()),
        signer: be_sdk_type_api::Signer::Verified,
    };
    for seq in 0..600 {
        sim.log.push(item(seq));
    }
    sim.open(0);
    sim.settle();
    // One appended past what the replica has read so far reaches it ahead of the rest.
    sim.log_item(item(600));
    sim.settle();
    let folded = Logbook {
        said: (0..=600).map(|seq: u64| seq.to_string()).collect(),
        ..Logbook::default()
    };
    assert_eq!(
        sim.devices[0]
            .described
            .as_ref()
            .map(|described| described.derived.clone()),
        Some(be_block_content::derived_metadata(&folded)),
        "the replica described a fold of part of its log"
    );
}
