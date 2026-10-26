use super::*;
use beui::reactive::*;
use beui::styled;
use beui_core::document::Document;
use beui_core::painter::Shape;

struct Shown(Rc<RefCell<Document>>);

impl App for Shown {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.borrow_mut().show(context, rect);
    }
}

fn rasters(output: &FrameOutput) -> Vec<f32> {
    output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { galley, .. } => Some(galley.pixels_per_point()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_region_drawn_at_a_finer_raster_paints_its_text_sharper_without_laying_it_out() {
    let _root = RootScope::new();
    let document = Rc::new(RefCell::new(build(move || {
        view! {
            <styled::Root>
                <styled::Text>"Sharper in space"</styled::Text>
            </styled::Root>
        }
    })));
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Shown(Rc::clone(&document)),
    );
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 40.0));
    surface.update(&region(rect, [120, 40]));
    let output = surface.take_output().expect("the first frame paints");
    assert_eq!(rasters(&output), [1.0], "text is drawn at the scale it is laid out at");

    let mut finer = region(rect, [360, 120]);
    finer.raster = 3.0;
    surface.update(&finer);
    let output = surface.take_output().expect("a new raster paints again");
    let work = document.borrow().performance().latest.work;
    assert_eq!(
        (work.measured, work.placed),
        (0, 0),
        "a raster alone lays nothing out again"
    );
    assert_eq!(output.main().pixels_per_point, 3.0);
    assert_eq!(
        output.main().size,
        rect.size(),
        "the region keeps its size in points"
    );
    assert_eq!(rasters(&output), [3.0], "its text is rasterised three times as finely");
}
