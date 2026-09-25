use super::*;

#[test]
fn zooming_the_view_grows_the_scene() {
    let mut editor = editor();
    let fitted = editor.rect_of("pixel_ray_tracer.artwork");

    editor.click("pixel_ray_tracer.fit");
    editor.run();
    editor.run();

    assert!(fitted.width() > 0.0, "the artwork is shown before zooming");
    editor.snapshot("zooming_the_view_grows_the_scene");
}
