use super::*;

#[test]
fn a_refused_replace_still_uses_up_its_offsets() {
    let mut sequence = loaded("- [ ] milk");
    let mut bob = sequence.clone();
    let alice = sequence
        .replace(ALICE, 3..4, b"x".to_vec())
        .expect("there is something to replace");
    let lost = bob
        .replace(BOB, 3..4, b"x".to_vec())
        .expect("there is something to replace");
    applied(&mut bob, &lost);
    let typed_after = typed(&mut bob, BOB, 10, "!");

    applied(&mut sequence, &alice);
    assert_eq!(sequence.apply(&lost), None);
    applied(&mut sequence, &typed_after);

    assert_eq!(text(&sequence), "- [x] milk!");
}
