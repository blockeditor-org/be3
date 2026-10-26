use super::*;

#[test]
fn the_hosts_own_windows_are_never_entries() {
    let desktop = FakeDesktop::new();
    desktop.add(RawWindow {
        process_id: OWN_PROCESS,
        ..window(1, "Block")
    });
    desktop.add(window(2, "other"));
    let mut model = Model::new();
    model.update(&desktop, &[]);
    assert_eq!(titles(&model.snapshot()), ["other"]);
}
