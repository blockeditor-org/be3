use super::*;

#[test]
fn popups_belong_to_their_owner_or_to_their_threads_window() {
    let desktop = FakeDesktop::new();
    let main = window(1, "main");
    let other = RawWindow {
        thread_id: main.thread_id,
        process_id: main.process_id,
        ..window(2, "second")
    };
    let menu = RawWindow {
        style: Style {
            popup: true,
            tool_window: true,
            ..Style::default()
        },
        frame: Rect::new(150, 150, 300, 400),
        ..owned(10, &main, "")
    };
    let tooltip = RawWindow {
        owner: None,
        style: Style {
            popup: true,
            tool_window: true,
            ..Style::default()
        },
        frame: Rect::new(160, 160, 260, 180),
        ..window(11, "")
    };
    let tooltip = RawWindow {
        thread_id: main.thread_id,
        process_id: main.process_id,
        ..tooltip
    };
    let stray = RawWindow {
        style: Style {
            tool_window: true,
            ..Style::default()
        },
        ..window(12, "stray tool")
    };
    for window in [
        tooltip.clone(),
        menu.clone(),
        main.clone(),
        other.clone(),
        stray,
    ] {
        desktop.add(window);
    }
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let snapshot = model.snapshot();
    assert_eq!(titles(&snapshot), ["main", "second"]);
    let popups: Vec<Handle> = snapshot.entries[0]
        .popups
        .iter()
        .map(|popup| popup.handle)
        .collect();
    assert_eq!(popups, [Handle(10), Handle(11)]);
    assert_eq!(snapshot.entries[0].popups[0].frame, menu.frame);
    assert!(snapshot.entries[1].popups.is_empty());

    desktop.foreground.set(Some(Handle(2)));
    model.update(&desktop, &[Event::Foreground(Handle(2))]);
    let snapshot = model.snapshot();
    let popups: Vec<Handle> = snapshot.entries[1]
        .popups
        .iter()
        .map(|popup| popup.handle)
        .collect();
    assert_eq!(popups, [Handle(11)]);
}
