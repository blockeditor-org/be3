use super::*;

const FRAMES: usize = 20;

#[test]
fn a_settled_editor_stops_laying_itself_out_again() {
    let (mut editor, _block, _host) = editor();
    for _ in 0..FRAMES {
        editor.run();
    }
    let settled = editor.document().performance();

    for _ in 0..FRAMES {
        editor.run();
    }
    let after = editor.document().performance();

    let laid_out =
        (after.samples - after.layout_cache_hits) - (settled.samples - settled.layout_cache_hits);
    let painted =
        (after.samples - after.paint_cache_hits) - (settled.samples - settled.paint_cache_hits);
    assert_eq!(
        (laid_out, painted),
        (0, 0),
        "an editor nobody is touching laid out {laid_out} and painted {painted} of {FRAMES} frames"
    );
}
