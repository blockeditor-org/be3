use beui::{Color32, FrameOutput, Vec2};
use beui_core::renderer::Renderer as _;
use beui_renderer_dom::{DomRenderer, MemoryDom};
use paint_snapshot::BrowserSnapshot;

pub(crate) struct DomPainting {
    renderer: DomRenderer<MemoryDom>,
}

impl DomPainting {
    pub(crate) fn new(size: Vec2) -> Self {
        Self {
            renderer: DomRenderer::new(MemoryDom::new(size.x, size.y))
                .expect("an in-memory DOM always has a stage"),
        }
    }

    pub(crate) fn show(&mut self, output: &FrameOutput, background: Color32) {
        self.renderer
            .prepare(output.main(), output.pixels_per_point(), background);
    }

    pub(crate) fn snapshot(
        &self,
        name: &str,
        output: &FrameOutput,
        size: Vec2,
        background: Color32,
    ) {
        let html = self.renderer.dom().html();
        let mut fresh = Self::new(size);
        fresh.show(output, background);
        assert!(
            html == fresh.renderer.dom().html(),
            "{name}: the DOM the renderer built frame by frame differs from the one it builds \
             from the last frame alone"
        );
        let mut snapshot = BrowserSnapshot::new([size.x as u32, size.y as u32], &html);
        let shown = self
            .renderer
            .dom()
            .images()
            .iter()
            .filter(|(url, _)| html.contains(url.as_str()));
        for (url, image) in shown {
            snapshot = snapshot.file(url, "image/png", &png(image));
        }
        snapshot.snapshot(&format!("{name}.browser"));
    }
}

fn png(image: &beui::Image) -> Vec<u8> {
    let mut png = Vec::new();
    image::RgbaImage::from_vec(image.width(), image.height(), image.pixels().to_vec())
        .expect("an image's pixels match its size")
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("an image always encodes as a PNG");
    png
}
