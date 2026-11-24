use super::*;

use be_block_content::blob::BlobOp;

/// A replica released with an editor's range read waiting on a part read carries no range to
/// the block's next replica: its editors are gone, and the part read is answered to its id,
/// which no later replica has. A replica opened after the release, under a new id, answers its
/// own editor's range once across a reopen, and the released one's range is answered to no one.
#[test]
fn a_range_read_under_a_replica_opened_after_a_release_is_answered_once_and_the_released_ones_to_no_one()
 {
    let mut sim = Sim::<FileContent>::new(1, file_module);
    sim.keeps_modules = true;
    let payload: Vec<u8> = (0..64 * 1024u32)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let recording = FileContent::new("take one".into(), payload.clone());
    sim.publish_elsewhere(&recording);
    let header = recording.encode().len() - payload.len();
    let read = |token| FromEditor::Read {
        token,
        offset: (header + 1000) as u64,
        length: 100,
    };
    sim.open(0);
    sim.attach(0, 1, true);

    // A range waiting on a part read the host has not answered, made first so the next
    // replica's read takes the same call, and an edit kept in the outbox keeps the released
    // replica's carry.
    sim.read_stalled = true;
    sim.send_from_editor(0, 1, read(7));
    sim.settle();
    sim.kept_publishes = true;
    sim.offline_submits = true;
    sim.send_from_editor(0, 1, FromEditor::FollowLacking);
    sim.settle();
    sim.operate(0, 1, BlobOp::SetHeader("take two".into()));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.lacking(0, 1), 1, "the edit is kept in the outbox");
    sim.close(0);
    assert!(sim.devices[0].released, "the replica was released");

    // The block opens again in the module under a new replica, whose editor reads the same
    // range while it opens again once more.
    sim.open_in_module(0);
    sim.attach(0, 2, true);
    sim.send_from_editor(0, 2, read(7));
    sim.settle();
    sim.reopen(0);
    sim.take_received(0, 2);
    sim.unstall();

    let wanted = payload[1000..1100].to_vec();
    let ranges: Vec<_> = sim
        .take_received(0, 2)
        .into_iter()
        .filter(|message| matches!(message, ToEditor::Range { .. }))
        .collect();
    assert!(
        matches!(ranges.as_slice(), [ToEditor::Range { token: 7, result: Ok(bytes) }] if *bytes == wanted),
        "the new replica's editor reads its range once: {ranges:?}"
    );
    let told: Vec<_> = sim
        .take_received(0, 1)
        .into_iter()
        .filter(|message| matches!(message, ToEditor::Range { .. }))
        .collect();
    assert!(
        told.is_empty(),
        "the released replica's range is answered to no one: {told:?}"
    );
}
