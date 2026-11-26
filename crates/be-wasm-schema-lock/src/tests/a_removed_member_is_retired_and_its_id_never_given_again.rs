use super::*;

#[test]
fn a_removed_member_is_retired_and_its_id_never_given_again() {
    let old = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    let removed = lock_of(
        &[("a.rs", "#[derive(Schema)] struct Person { age: u32 }")],
        &old,
    )
    .unwrap();
    assert_eq!(
        ids(&removed, "Person"),
        [(1, "name".into(), true), (2, "age".into(), false)]
    );
    let back = lock_of(&[("a.rs", PERSON)], &removed).unwrap();
    assert_eq!(
        ids(&back, "Person"),
        [
            (1, "name".into(), true),
            (2, "age".into(), false),
            (3, "name".into(), false)
        ]
    );
}
