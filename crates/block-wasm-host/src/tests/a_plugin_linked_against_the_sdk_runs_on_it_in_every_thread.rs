use super::*;

#[test]
fn a_plugin_linked_against_the_sdk_runs_on_it_in_every_thread() {
    let identity = sdk_identity();
    let mut plugin = host()
        .load_bytes_with_sdk(linked_plugin(&identity).as_bytes(), |named| {
            assert_eq!(named, identity);
            Ok(sdk().as_bytes().to_vec())
        })
        .unwrap();
    plugin.start().unwrap();
    assert_eq!(settled(&mut plugin, 7 + 200 + 5), Ok(()));
}
