use super::*;

#[test]
fn both_sides_setting_a_field_to_the_same_value_merge_without_a_conflict() {
    let base = board();
    let (_, _, write) = ids(&base);
    let ours = edited(&base, [Card::TEXT.set(write, &"write it".to_owned())]);
    let theirs = edited(&base, [Card::TEXT.set(write, &"write it".to_owned())]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(merged, ours);
}
