use super::*;

#[test]
fn a_library_that_is_not_what_it_is_named_is_refused() {
    let identity = "0".repeat(64);
    let Err(error) = host().load_bytes_with_libraries(linked_plugin(&identity).as_bytes(), |_| {
        Ok(root().as_bytes().to_vec())
    }) else {
        panic!("a library whose SHA-256 is not its name was linked");
    };
    assert!(error.contains("its SHA-256 is"), "{error}");
}
