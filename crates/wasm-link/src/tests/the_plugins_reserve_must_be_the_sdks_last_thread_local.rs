use super::*;

#[test]
fn the_plugins_reserve_must_be_the_sdks_last_thread_local() {
    let found = map::thread_locals(MAP).unwrap();
    assert!(
        sdk_listing(&[], &found)
            .unwrap_err()
            .contains("is not the last")
    );
}
