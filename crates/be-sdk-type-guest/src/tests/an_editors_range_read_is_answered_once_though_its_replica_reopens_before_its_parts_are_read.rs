use super::*;

/// A range an editor reads that waits on a part read as its replica opens again is answered
/// by the new replica from that part read's answer, as planned against the content it was
/// asked of, and the part is read once.
#[test]
fn an_editors_range_read_is_answered_once_though_its_replica_reopens_before_its_parts_are_read() {
    let mut sim = Sim::<FileContent>::new(1, file_module);
    let payload: Vec<u8> = (0..64 * 1024u32)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let recording = FileContent::new("take one".into(), payload.clone());
    sim.publish_elsewhere(&recording);
    let header = recording.encode().len() - payload.len();
    sim.open(0);
    sim.attach(0, 1, true);
    sim.take_received(0, 1);

    sim.read_stalled = true;
    sim.send_from_editor(
        0,
        1,
        FromEditor::Read {
            token: 7,
            offset: (header + 1000) as u64,
            length: 100,
        },
    );
    sim.settle();
    sim.reopen(0);
    sim.unstall();

    let wanted = payload[1000..1100].to_vec();
    let ranges: Vec<_> = sim
        .take_received(0, 1)
        .into_iter()
        .filter(|message| matches!(message, ToEditor::Range { token: 7, .. }))
        .collect();
    assert!(
        matches!(ranges.as_slice(), [ToEditor::Range { result: Ok(bytes), .. }] if *bytes == wanted),
        "the editor's range is answered once, with what it asked for: {ranges:?}"
    );
    assert_eq!(sim.part_reads, 1, "the part was read once");
}
