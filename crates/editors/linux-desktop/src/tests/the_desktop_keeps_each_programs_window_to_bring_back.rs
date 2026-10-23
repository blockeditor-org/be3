use block_editor_beui::beui::{Pos2, Vec2};

use super::*;

#[test]
fn the_desktop_keeps_each_programs_window_to_bring_back() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let full = HostWindow {
        fullscreen: Some(Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        )),
        ..notes(2)
    };
    let dialog = HostWindow {
        parent: Some(HostWindowId(2)),
        title: "Save changes?".to_owned(),
        ..notes(3)
    };
    fixture.show_windows(&[foot(1), full, dialog]);

    assert_eq!(
        fixture.kept(),
        vec![
            KeptWindow {
                key: HostWindowId(1),
                window: Some(HostWindowId(1)),
                app_id: "foot".to_owned(),
                title: "Terminal 1".to_owned(),
                origin: foot(1).origin,
                fullscreen: false,
            },
            KeptWindow {
                key: HostWindowId(2),
                window: Some(HostWindowId(2)),
                app_id: "org.example.Notes".to_owned(),
                title: "Shopping list".to_owned(),
                origin: notes(2).origin,
                fullscreen: true,
            },
        ],
        "each top-level window is kept with what brings it back, and a dialog is not"
    );

    fixture.show_windows(&[foot(1)]);
    assert_eq!(
        kept_keys(&fixture),
        vec![1],
        "a window that closes is no longer kept"
    );
}
