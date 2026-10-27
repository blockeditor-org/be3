use super::*;

#[test]
fn focus_follows_the_foreground_and_counts_raises() {
    let desktop = FakeDesktop::new();
    let main = window(1, "main");
    desktop.add(RawWindow {
        style: Style {
            popup: true,
            tool_window: true,
            ..Style::default()
        },
        ..owned(10, &main, "menu")
    });
    desktop.add(main);
    desktop.add(window(2, "other"));
    let mut model = Model::new();
    desktop.foreground.set(Some(Handle(1)));
    model.update(&desktop, &[]);
    let focused = |model: &Model| -> Vec<(bool, u64)> {
        model
            .snapshot()
            .entries
            .iter()
            .map(|entry| (entry.focused, entry.raised))
            .collect()
    };
    assert_eq!(focused(&model), [(true, 1), (false, 0)]);

    desktop.foreground.set(Some(Handle(10)));
    model.update(&desktop, &[Event::Foreground(Handle(10))]);
    assert_eq!(focused(&model), [(true, 1), (false, 0)]);

    desktop.foreground.set(Some(Handle(2)));
    model.update(&desktop, &[Event::Foreground(Handle(2))]);
    assert_eq!(focused(&model), [(false, 1), (true, 1)]);

    desktop.foreground.set(Some(Handle(1)));
    model.update(&desktop, &[Event::Foreground(Handle(1))]);
    assert_eq!(focused(&model), [(true, 2), (false, 1)]);
}
