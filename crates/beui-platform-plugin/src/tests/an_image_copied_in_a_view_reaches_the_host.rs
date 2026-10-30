use super::*;

#[test]
fn an_image_copied_in_a_view_reaches_the_host() {
    let host = EditorHost::default();
    let mut png = Some(vec![137, 80, 78, 71]);
    let mut surface = launched(
        &host,
        Windowed::empty().hooked(move |context| {
            if let Some(png) = png.take() {
                context.copy_image(png);
            }
        }),
    );
    one(&mut surface, &whole(40.0, 30.0));
    assert_eq!(host.take_copied_images(), vec![vec![137, 80, 78, 71]]);
    assert!(host.take_copied_text().is_empty());
}
