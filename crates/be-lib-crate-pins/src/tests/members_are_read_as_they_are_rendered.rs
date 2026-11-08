use super::*;

#[test]
fn members_are_read_as_they_are_rendered() {
    let members = vec!["crates/a".to_owned(), "crates/b-c".to_owned()];
    assert_eq!(super::members(&render_members(&members)), members);
}
