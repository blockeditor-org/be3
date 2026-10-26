use super::*;

struct CopiesImage(Option<Vec<u8>>);

impl App for CopiesImage {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if let Some(png) = self.0.take() {
            context.copy_image(png);
        }
    }
}

#[test]
fn an_image_copied_in_a_region_reaches_the_host() {
    let host = EditorHost::default();
    let mut surface = launched(
        &host,
        beui::context(),
        CopiesImage(Some(vec![137, 80, 78, 71])),
    );
    let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0));
    surface.update(&region(whole, [40, 30]));
    assert_eq!(host.take_copied_images(), vec![vec![137, 80, 78, 71]]);
    assert!(host.take_copied_text().is_empty());
}
