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
    assert!(
        placed_windows(&fixture).is_empty(),
        "the window is not shown while its program closes"
    );

    fixture.host.set_windows(vec![
        window(3, "Terminal", None),
        window(4, "Really quit?", Some(3)),
    ]);
    fixture.settle();

    let mut placed: Vec<u64> = placed_windows(&fixture)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    placed.sort_unstable();
    assert_eq!(
        placed,
        vec![3, 4],
        "a program that asks before closing gets its window back beside the question"
    );
}
