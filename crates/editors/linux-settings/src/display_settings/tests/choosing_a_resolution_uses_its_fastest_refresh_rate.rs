use super::*;

#[test]
fn choosing_a_resolution_uses_its_fastest_refresh_rate() {
    let mut editor = editor(vec![gaming_monitor()]);

    choose(&mut editor, RESOLUTION, 1);
    assert_eq!(
        content(&editor).root().mode(MONITOR),
        Some(saved(1920, 1080, 240_000))
    );

    choose(&mut editor, REFRESH, 1);
    assert_eq!(
        content(&editor).root().mode(MONITOR),
        Some(saved(1920, 1080, 60_000)),
        "the refresh rates offered are the chosen resolution's"
    );
}
