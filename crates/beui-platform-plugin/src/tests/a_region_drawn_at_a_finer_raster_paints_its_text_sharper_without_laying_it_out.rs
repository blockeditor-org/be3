use super::*;
use beui_core::painter::Shape;

fn rasters(surface: &ModuleRunner, region: &Region) -> (f32, Vec<f32>) {
    let window = surface
        .window(region.id())
        .expect("the region is a window of the app");
    let output = surface
        .runner
        .output()
        .and_then(|output| output.window(window))
        .expect("the region's window painted");
    let texts = output
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { galley, .. } => Some(galley.pixels_per_point()),
            _ => None,
        })
        .collect();
    (output.pixels_per_point, texts)
}

#[test]
fn a_region_drawn_at_a_finer_raster_paints_its_text_sharper_without_laying_it_out() {
    let _root = RootScope::new();
    let document = Rc::default();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(|_| {
            view! {
                <styled::Text>"Sharper in space"</styled::Text>
            }
        })
        .sharing(&document),
    );
    let region = whole(120.0, 40.0);
    one(&mut surface, &region);
    one(&mut surface, &region);
    assert_eq!(
        rasters(&surface, &region),
        (1.0, vec![1.0]),
        "text is drawn at the scale it is laid out at"
    );

    let mut finer = region.clone();
    finer.raster = 3.0;
    finer.pixels = [360, 120];
    let frame = one(&mut surface, &finer);
    assert!(frame.changed, "a new raster paints the region again");
    let work = document
        .borrow()
        .as_ref()
        .map(|document: &Document| document.performance().latest.work)
        .expect("the app built its document");
    assert_eq!(
        (work.measured, work.placed),
        (0, 0),
        "a raster alone lays nothing out again"
    );
    assert_eq!(
        rasters(&surface, &finer),
        (3.0, vec![3.0]),
        "its window and its text are rasterised three times as finely"
    );
    let painting = surface
        .painting(finer.id())
        .expect("the region paints at its raster");
    assert_eq!(painting.frame.size, [360, 120]);
}
