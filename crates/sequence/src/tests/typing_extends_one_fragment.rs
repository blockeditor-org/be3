use super::*;

#[test]
fn typing_extends_one_fragment() {
    let mut sequence = loaded("ab");

    for (at, typed_character) in ["x", "y", "z"].into_iter().enumerate() {
        typed(&mut sequence, ALICE, 1 + at, typed_character);
    }

    assert_eq!(text(&sequence), "axyzb");
    assert_eq!(sequence.fragment_count(), 3);
    assert!(!sequence.is_fresh());
}
