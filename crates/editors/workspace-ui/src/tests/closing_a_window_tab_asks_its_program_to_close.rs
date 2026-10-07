use super::*;

#[test]
fn closing_a_window_tab_asks_its_program_to_close() {
    let (mut fixture, _) = editor();
    fixture.host.set_windows(vec![window(3, "Terminal", None)]);
    fixture.settle();

    let tab = (1u64 << 41) + 3;
    fixture.test.click(&format!("dock.tab.{tab}.close"));
    fixture.settle();

    assert!(
        fixture.test.sent().iter().any(|message| matches!(
            message,
            block_plugin_api::EditorMessage::CloseWindow {
                window: block_editor_beui::HostWindowId(3),
                ..
            }
        )),
        "the host is asked to close the window"
    );
    assert_eq!(
        placed_windows(&fixture)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![3],
        "the tab stays while its program decides"
    );

    fixture.host.set_windows(Vec::new());
    fixture.settle();

    assert!(
        placed_windows(&fixture).is_empty(),
        "the tab goes when its program closes the window"
    );
}
