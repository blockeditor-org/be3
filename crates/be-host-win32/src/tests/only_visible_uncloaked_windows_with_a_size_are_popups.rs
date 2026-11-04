use super::*;

#[test]
fn only_visible_uncloaked_windows_with_a_size_are_popups() {
    let desktop = FakeDesktop::new();
    let main = window(1, "main");
    let popup = |handle: u64, frame: Rect| RawWindow {
        style: Style {
            popup: true,
            tool_window: true,
            ..Style::default()
        },
        frame,
        ..owned(handle, &main, "")
    };
    let menu = popup(10, Rect::new(150, 150, 300, 400));
    let hidden = RawWindow {
        visible: false,
        ..popup(11, Rect::new(150, 150, 300, 400))
    };
    let cloaked = RawWindow {
        cloaked: true,
        ..popup(12, Rect::new(150, 150, 300, 400))
    };
    let empty = popup(13, Rect::new(150, 150, 150, 400));
    let same_thread_empty = RawWindow {
        owner: None,
        thread_id: main.thread_id,
        process_id: main.process_id,
        ..popup(14, Rect::new(10, 10, 10, 10))
    };
    let minimized = RawWindow {
        minimized: true,
        ..popup(15, Rect::new(150, 150, 300, 400))
    };
    for window in [
        menu,
        hidden,
        cloaked,
        empty,
        same_thread_empty,
        minimized,
        main.clone(),
    ] {
        desktop.add(window);
    }
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let snapshot = model.snapshot();
    assert_eq!(titles(&snapshot), ["main"]);
    let popups: Vec<Handle> = snapshot.entries[0]
        .popups
        .iter()
        .map(|popup| popup.handle)
        .collect();
    assert_eq!(popups, [Handle(10)]);
}
