use super::*;

#[test]
fn a_library_the_link_does_not_name_is_refused() {
    let error = closure(vec![library("left", &["base"], &[], &[])]).unwrap_err();
    assert!(error.contains("left needs the library base"), "{error}");
}
