use super::*;

/// A follower's paste over `LARGEST_RELAYED_OPERATION` goes as any other operation, by the
/// part its bytes were put as: the owner sequences it among its own unsealed edits, every
/// replica reads the part and applies it where it was made, and undo takes back the paste.
#[test]
fn a_followers_paste_too_large_to_relay_lands_once_over_the_owners_unsealed_edits_and_undoes_alone()
{
    let mut sim = Sim::<ProseContent>::new(3, prose_with_undo_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.type_at(0, 1, 6, "owner\n");
    sim.settle();
    sim.type_at(1, 1, 12, "typed\n");
    sim.settle();
    sim.gesture(1, 1, Gesture::Break);
    let published = sim.publishes;
    assert!(!sim.devices[0].status.clean, "the owner has unsealed edits");

    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(1, 1, 18, &pasted);
    sim.type_at(0, 1, 0, "> ");
    sim.settle();
    assert_eq!(sim.parts_put, 1, "the paste was put as a part");
    assert_eq!(
        sim.part_reads, 2,
        "the owner and the other follower read it"
    );
    assert!(
        sim.largest_relay < 1024 * 1024,
        "a relayed message carried {} bytes",
        sim.largest_relay
    );
    let expected = format!("> start\nowner\ntyped\n{pasted}");
    for device in 0..3 {
        assert!(prose(&sim.content(device)) == expected, "device {device}");
        assert!(sim.typed(device, 1) == expected, "device {device}");
    }
    sim.advance(0);
    assert_eq!(sim.publishes, published + 1, "the owner committed at once");
    assert!(prose(&sim.head_content()) == expected);

    assert!(sim.change(1, Change::Undo));
    sim.settle();
    let undone = "> start\nowner\ntyped\n";
    for device in 0..3 {
        assert_eq!(prose(&sim.content(device)), undone, "device {device}");
        assert_eq!(sim.typed(device, 1), undone, "device {device}");
    }
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.head_content()), undone);
    assert!(sim.conflicts.is_empty());
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
