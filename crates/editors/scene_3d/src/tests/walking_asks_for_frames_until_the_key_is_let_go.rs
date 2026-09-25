use super::*;

#[test]
fn walking_asks_for_frames_until_the_key_is_let_go() {
    let (mut scene, _host, region) = scene();
    scene.input(&region, &key(Key::W, true));
    assert_eq!(
        scene.update(&region, None).repaint_after,
        None,
        "keys do nothing until the scene is clicked"
    );

    scene.input(&region, &click());
    scene.input(&region, &key(Key::W, true));
    assert_eq!(
        scene.update(&region, None).repaint_after,
        Some(Duration::ZERO)
    );

    scene.input(&region, &key(Key::W, false));
    assert_eq!(scene.update(&region, None).repaint_after, None);
}
