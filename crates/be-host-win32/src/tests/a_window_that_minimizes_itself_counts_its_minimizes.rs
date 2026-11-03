use super::*;

#[test]
fn a_window_that_minimizes_itself_counts_its_minimizes() {
    let desktop = FakeDesktop::new();
    desktop.add(RawWindow {
        minimized: true,
        ..window(1, "started minimized")
    });
    desktop.add(window(2, "paint"));
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let minimizes = |model: &Model| -> Vec<u64> {
        model
            .snapshot()
            .entries
            .iter()
            .map(|entry| entry.minimizes)
            .collect()
    };
    assert_eq!(
        minimizes(&model),
        [0, 0],
        "a window first seen minimized never minimized itself"
    );

    desktop.change(Handle(2), |window| window.minimized = true);
    model.update(&desktop, &[Event::Minimized(Handle(2))]);
    model.update(&desktop, &[]);
    assert_eq!(minimizes(&model), [0, 1], "it counts once per minimize");

    desktop.change(Handle(2), |window| window.minimized = false);
    model.update(&desktop, &[Event::Restored(Handle(2))]);
    desktop.change(Handle(2), |window| window.minimized = true);
    model.update(&desktop, &[Event::Minimized(Handle(2))]);
    assert_eq!(minimizes(&model), [0, 2]);
}
