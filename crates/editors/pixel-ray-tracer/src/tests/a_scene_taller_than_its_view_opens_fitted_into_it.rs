use super::*;

#[test]
fn a_scene_taller_than_its_view_opens_fitted_into_it() {
    let mut editor = editor_sized(Vec2::new(900.0, 340.0));

    let band = editor
        .content_rect()
        .expect("the editor reports its content");
    let artwork = editor.rect_of("pixel_ray_tracer.artwork");
    assert!(
        artwork.height() < 384.0 && band.expand(0.5).contains_rect(artwork),
        "the editor zooms its 384-point scene out to fit, {artwork:?} in {band:?}"
    );
    assert!(
        (artwork.center() - band.center()).length() < 1.0,
        "and centres it, {artwork:?} in {band:?}"
    );
    editor.snapshot("a_scene_taller_than_its_view_opens_fitted_into_it");
}
