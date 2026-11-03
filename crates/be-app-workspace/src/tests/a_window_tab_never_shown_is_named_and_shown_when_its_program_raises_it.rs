use super::*;

#[test]
fn a_window_tab_never_shown_is_named_and_shown_when_its_program_raises_it() {
    let (mut fixture, _) = editor();
    let windows = [
        window(7, "notes - Text Editor", None),
        window(8, "~ - Terminal", None),
    ];
    fixture.show_windows(&windows);
    fixture.settle();

    let placed: Vec<u64> = placed_windows(&fixture).iter().map(|(id, _)| *id).collect();
    let [shown] = placed[..] else {
        panic!("one window tab is shown at a time, got {placed:?}");
    };
    let unshown = if shown == 7 { 8 } else { 7 };
    assert!(
        fixture.says("Text Editor \u{2014} notes") && fixture.says("Terminal \u{2014} ~"),
        "both tabs carry their window's title, the one never shown too"
    );

    fixture.show_windows(&windows.clone().map(|held| match held.id == unshown {
        true => TestWindow { raised: 1, ..held },
        false => held,
    }));
    fixture.settle();
    assert_eq!(
        placed_windows(&fixture)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![unshown],
        "a window its program raises shows its tab"
    );
}
