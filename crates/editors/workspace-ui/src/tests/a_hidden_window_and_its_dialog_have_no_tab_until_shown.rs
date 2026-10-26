use super::*;

#[test]
fn a_hidden_window_and_its_dialog_have_no_tab_until_shown() {
    let (mut fixture, _) = editor();
    let hidden = block_editor_beui::HostWindow {
        shown: false,
        ..window(5, "Paint", None)
    };
    fixture.host.set_watched::<WindowsWatch>(&vec![
        hidden.clone(),
        window(6, "Save as", Some(5)),
        window(7, "Terminal", None),
    ]);
    fixture.settle();
    let placed: Vec<u64> = placed_windows(&fixture).iter().map(|(id, _)| *id).collect();
    assert!(
        !placed.contains(&5) && !placed.contains(&6),
        "neither a hidden window nor its dialog is drawn: {placed:?}"
    );
    assert!(fixture.says("Terminal"));
    assert!(!fixture.says("Paint"));
    assert!(!fixture.says("Save as"));

    fixture.host.set_watched::<WindowsWatch>(&vec![
        block_editor_beui::HostWindow {
            shown: true,
            ..hidden
        },
        window(6, "Save as", Some(5)),
        window(7, "Terminal", None),
    ]);
    fixture.settle();
    assert!(
        fixture.says("Paint"),
        "shown again, the window has its tab back"
    );
    assert!(fixture.says("Save as"), "and its dialog is back");
}
