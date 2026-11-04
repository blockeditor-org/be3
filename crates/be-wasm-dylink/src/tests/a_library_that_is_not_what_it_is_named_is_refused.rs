use super::*;

#[test]
fn a_library_that_is_not_what_it_is_named_is_refused() {
    let bytes = b"\0asm\x01\0\0\0";
    assert_eq!(check_library(bytes, &identity(bytes)), Ok(()));
    let error = check_library(bytes, &"0".repeat(64)).unwrap_err();
    assert!(error.contains(&identity(bytes)), "{error}");
}
