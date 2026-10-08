use super::*;

#[test]
fn inserts_after_the_same_element_put_the_later_one_first() {
    let mut sequence = loaded("ab");
    let mut alice = sequence.clone();
    let mut bob = sequence.clone();
    let alice_first = typed(&mut alice, ALICE, 1, "X");
    let bob_first = typed(&mut bob, BOB, 1, "Y");
    let alice_second = typed(&mut alice, ALICE, 2, "X");
    let bob_second = typed(&mut bob, BOB, 2, "Y");

    for op in [&alice_first, &bob_first, &alice_second, &bob_second] {
        applied(&mut sequence, op);
    }

    assert_eq!(text(&sequence), "aYYXXb");
}
