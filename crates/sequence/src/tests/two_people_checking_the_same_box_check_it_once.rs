use super::*;

#[test]
fn two_people_checking_the_same_box_check_it_once() {
    let mut sequence = loaded("- [ ] milk");
    let alice = sequence
        .replace(ALICE, 3..4, b"x".to_vec())
        .expect("there is something to replace");
    let bob = sequence
        .replace(BOB, 3..4, b"x".to_vec())
        .expect("there is something to replace");

    applied(&mut sequence, &alice);
    assert_eq!(sequence.apply(&bob), None);
    assert_eq!(text(&sequence), "- [x] milk");

    let unchecked = sequence
        .replace(BOB, 3..4, b" ".to_vec())
        .expect("there is something to replace");
    applied(&mut sequence, &unchecked);
    assert_eq!(text(&sequence), "- [ ] milk");
}
