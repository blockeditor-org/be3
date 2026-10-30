use super::*;

#[test]
fn a_plugin_linked_against_a_library_runs_on_it_in_every_thread() {
    let identity = root_identity();
    let mut plugin = host()
        .load_bytes_with_libraries(linked_plugin(&identity).as_bytes(), |named| {
            assert_eq!(named, identity);
            Ok(root().as_bytes().to_vec())
        })
        .unwrap();
    plugin.start().unwrap();
    assert_eq!(settled(&mut plugin, 7 + 200 + 5), Ok(true));
}
