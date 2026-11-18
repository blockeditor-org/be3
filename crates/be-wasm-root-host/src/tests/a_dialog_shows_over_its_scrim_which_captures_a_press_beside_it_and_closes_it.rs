use be_lib_geometry::{Rect, pos2};
use be_lib_layers::Area;
use be_sdk_root_testing::{
    DIALOG_LAYER, DIALOG_ORIGIN, DIALOG_SIZE, DIALOG_SURFACE, ROOT_LAYER, SCRIM_LAYER,
};

use super::*;

#[test]
fn a_dialog_shows_over_its_scrim_which_captures_a_press_beside_it_and_closes_it() {
    let mut root = embedding();
    let mut time = 1;
    let first = step(&mut root, time);
    assert!(
        layer_of(&first, SCRIM_LAYER).is_none() && layer_of(&first, DIALOG_LAYER).is_none(),
        "no dialog shows until one opens"
    );

    let open = open_dialog(&mut root, &mut time);
    let layers = shown(&open);
    assert_eq!(
        layers[layers.len() - 2..],
        [SCRIM_LAYER, DIALOG_LAYER],
        "the dialog shows over its scrim, over everything else"
    );
    let scrim = layer_of(&open, SCRIM_LAYER).expect("the scrim shows");
    assert_eq!(
        (
            &scrim.input.hit,
            scrim.input.captures,
            scrim.input.focuses,
            scrim.input.placer,
        ),
        (&Area::Nowhere, true, false, Some(ROOT_LAYER)),
        "the scrim takes no press itself, but captures every press that misses the dialog for \
         the root's own layer, and never takes the focus"
    );
    let dialog = Rect::from_min_size(
        pos2(DIALOG_ORIGIN.0, DIALOG_ORIGIN.1),
        be_lib_geometry::vec2(DIALOG_SIZE.0 as f32, DIALOG_SIZE.1 as f32),
    );
    let ime = be_lib_ui_types::ImeArea::decode(open.ime.as_deref().expect("the field edits text"))
        .expect("the input method's area reads back");
    assert!(
        dialog.contains_rect(ime.rect),
        "the dialog took the focus, and the input method sits on its field, in the window's \
         points: {:?} in {dialog:?}",
        ime.rect
    );

    let painting = own_paintings(&mut root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(DIALOG_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its dialog");
    beui_paint_snapshot::assert_painting(
        "a_dialog_shows_over_its_scrim_which_captures_a_press_beside_it_and_closes_it",
        painting,
    );

    // A press on the dialog, by its title, is the dialog's.
    let on_title = pos2(DIALOG_ORIGIN.0 + 60.0, DIALOG_ORIGIN.1 + 30.0);
    root.input(None, be_lib_ui_types::input::encode(&click(on_title)))
        .expect("the root takes input");
    for _ in 0..4 {
        time += 1;
        let drawn = step(&mut root, time);
        assert!(
            layer_of(&drawn, DIALOG_LAYER).is_some(),
            "a press on the dialog leaves it open"
        );
    }

    // Beside it, the scrim captures the press for the root, which closes the dialog.
    let closed = until(
        &mut root,
        &mut time,
        click(pos2(10.0, 10.0)),
        "a press on the scrim beside the dialog closes it",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(
        layer_of(&closed, SCRIM_LAYER).is_none(),
        "the scrim goes with the dialog"
    );
    assert!(
        closed.ime.is_none(),
        "with the dialog gone, nothing edits text"
    );
}
