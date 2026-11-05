use super::*;

use be_block::blob::BlobOp;

fn payload() -> Vec<u8> {
    (0..64 * 1024u32)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect()
}

#[test]
fn a_replica_that_loads_only_the_header_reads_the_payload_on_demand() {
    let mut sim = Sim::<RecordingContent>::new(2, recording_module);
    let payload = payload();
    let recording = RecordingContent::new("take one".into(), payload.clone());
    sim.publish_elsewhere(&recording);
    let header = recording.encode().len() - payload.len();

    sim.open(0);
    sim.attach(0, 1, true);
    let shown = sim.mirror(0, 1).clone();
    assert_eq!(shown.header(), "take one");
    assert!(shown.data().is_empty(), "the editor was sent the payload");
    assert_eq!(
        sim.editors[&(0, 1)].stored_length(),
        recording.encode().len() as u64
    );
    assert_eq!(sim.part_reads, 0, "opening read the payload");

    let offset = (header + 1000) as u64;
    sim.send_from_editor(
        0,
        1,
        FromEditor::Read {
            token: 7,
            offset,
            length: 100,
        },
    );
    sim.settle();
    let wanted = payload[1000..1100].to_vec();
    assert!(
        sim.take_received(0, 1).iter().any(|message| matches!(
            message,
            ToEditor::Range { token: 7, result: Ok(bytes) } if *bytes == wanted
        )),
        "the editor read a range of the payload"
    );
    assert_eq!(sim.part_reads, 1);

    sim.open(1);
    sim.attach(1, 1, true);
    assert_eq!(sim.mirror(1, 1).header(), "take one");
    assert!(sim.mirror(1, 1).data().is_empty());
    assert!(
        sim.largest_relay < 4096,
        "a joining follower was sent {} bytes",
        sim.largest_relay
    );
    assert_eq!(sim.part_reads, 1, "joining read the payload");

    sim.operate(1, 1, BlobOp::SetHeader("take two".into()));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
    let renamed = RecordingContent::new("take two".into(), payload.clone());
    assert_eq!(sim.head_content(), renamed);
    assert_eq!(sim.part_reads, 1, "saving a rename read the payload");

    assert_eq!(
        sim.content(0),
        renamed,
        "content reads what it left in the store"
    );
}
