use super::*;

#[test]
fn a_follower_typing_while_the_owner_types_keeps_both_runs_whole() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("base"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    let letters = "abcdefghijklmnopqrstuvwxyzabcdefghijklmn";
    let digits = "0123456789012345678901234567890123456789";
    for at in 0..letters.len() {
        sim.type_at(1, 1, at, &letters[at..=at]);
        let end = sim.typed(0, 1).len();
        sim.type_at(0, 1, end, &digits[at..=at]);
    }
    sim.settle();
    let expected = format!("{letters}base{digits}");
    for device in 0..2 {
        assert_eq!(prose(&sim.content(device)), expected);
        assert_eq!(sim.typed(device, 1), expected);
    }
    assert!(sim.devices[1].status.clean);
}
