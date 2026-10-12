use super::*;

#[test]
fn a_plugin_linked_against_the_sdk_runs_on_it_in_every_thread() {
    let mut plugin = host()
        .load_bytes_with_sdk(linked_plugin().as_bytes(), |identity| {
            assert_eq!(identity, SDK_IDENTITY);
            Ok(sdk().as_bytes().to_vec())
        })
        .unwrap();
    plugin.start().unwrap();
    assert_eq!(settled(&mut plugin, 7 + 200 + 5), Ok(true));
}
