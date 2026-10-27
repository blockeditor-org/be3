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
    model.update(
        &desktop,
        &[Event::Shown(Handle(1)), Event::NameChanged(Handle(1))],
    );
    assert_eq!(titles(&model.snapshot()), ["other"]);
    let reads = desktop.reads.borrow();
    assert!(
        reads.iter().all(|read| match *read {
            Read::Title(handle) | Read::Icon(handle) | Read::AppId(handle) => handle != Handle(1),
        }),
        "the host's own window was read: {reads:?}"
    );
    assert!(reads.contains(&Read::Title(Handle(2))));
}
