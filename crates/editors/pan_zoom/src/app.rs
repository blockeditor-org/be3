use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use block_client::blocks::pan_zoom::PanZoom as PanZoomBlock;
use block_editor_plugin::EditorHost;
use block_editor_plugin::beui::{Document, NodeId, Rect, Vec2};

mod ui;

use ui::{PanZoomUi, Viewport};

#[derive(Default)]
pub struct PanZoomApp {
    ui: Option<PanZoomUi>,
    host: Option<EditorHost>,
    viewport: Option<Rc<HostViewport>>,
    creation: Option<Arc<block_client::BlockClient>>,
}

impl PanZoomApp {
    pub fn canvas(&self) -> Option<Rect> {
        self.viewport.as_ref().map(|viewport| viewport.canvas.get())
    }
}

struct HostViewport {
    host: EditorHost,
    canvas: Cell<Rect>,
}

impl Viewport for HostViewport {
    fn zoom(&self, factor: f32) {
        self.host.beui_view().zoom(factor, None);
    }

    fn fit(&self) {
        self.host.beui_view().fit();
    }

    fn focus(&self, target: Rect) {
        let view = self.host.beui_view();
        let Some(placement) = view.canvas() else {
            return;
        };
        let shown = placement.rect_to_screen(target).center();
        view.pan(self.canvas.get().center() - shown);
    }
}

impl block_editor_plugin::BeuiApp for PanZoomApp {
    fn connect(
        &mut self,
        host: EditorHost,
        _client: Arc<block_client::BlockClient>,
        _block_id: uuid::Uuid,
    ) {
        let viewport = Rc::new(HostViewport {
            host: host.clone(),
            canvas: Cell::new(Rect::ZERO),
        });
        self.viewport = Some(viewport);
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<block_client::BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<uuid::Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(PanZoomBlock::default()).id())
    }

    fn intrinsic_size(&mut self) -> Option<Vec2> {
        Some(ui::world_size())
    }

    fn view(&mut self) -> NodeId {
        let viewport = self
            .viewport
            .clone()
            .expect("connect is called before view is built");
        let (ui, root) = PanZoomUi::new(viewport);
        self.ui = Some(ui);
        root
    }

    fn update(&mut self) {
        let (Some(host), Some(ui)) = (self.host.as_ref(), self.ui.as_mut()) else {
            return;
        };
        let view = host.beui_view();
        ui.set_view(view.canvas(), view.scale(), host.chrome_shown());
    }

    fn after_layout(&mut self, document: &Document) {
        let (Some(host), Some(ui), Some(viewport)) =
            (self.host.as_ref(), self.ui.as_ref(), self.viewport.as_ref())
        else {
            return;
        };
        let canvas = ui.canvas_rect(document).unwrap_or(viewport.canvas.get());
        viewport.canvas.set(canvas);
        host.beui_view().set_content(canvas);
    }
}
