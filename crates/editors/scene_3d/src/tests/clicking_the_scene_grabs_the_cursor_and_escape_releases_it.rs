use super::*;

#[test]
fn clicking_the_scene_grabs_the_cursor_and_escape_releases_it() {
    let (mut scene, host, region) = scene();
    scene.update(&region, None);
    assert!(!host.cursor_grabbed());

    scene.input(&region, &click());
    assert!(host.cursor_grabbed());
    assert_eq!(host.take_cursor_grab(), Some(true));

    scene.input(&region, &key(Key::Escape, true));
    assert!(!host.cursor_grabbed());
    assert_eq!(host.take_cursor_grab(), Some(false));
}
