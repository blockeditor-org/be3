use super::*;
use beui::reactive::{Frame, ItemSize, NodeRef, Row, percent, size, view};

#[derive(Default)]
struct SidebarApp {
    host: Option<EditorHost>,
    canvas: NodeRef,
}

impl crate::BeuiApp for SidebarApp {
    fn connect(&mut self, host: EditorHost, _client: Arc<BlockClient>, _block_id: Uuid) {
        self.host = Some(host);
    }

    fn view(&mut self) -> beui::NodeId {
        let canvas = NodeRef::new();
        self.canvas = canvas.clone();
        let sidebar = view! {
            <Frame />
        };
        let stage = view! {
            <Frame @node_ref={&canvas} />
        };
        let children = vec![size(sidebar, ItemSize::Fixed(100.0)), percent(stage, 100.0)];
        view! {
            <Row spacing=0.0 children={children} />
        }
    }

    fn after_layout(&mut self, document: &beui::Document) {
        let (Some(host), Some(canvas)) = (self.host.as_ref(), self.canvas.try_get()) else {
            return;
        };
        if let Some(rect) = document.node_rect(canvas) {
            host.beui_view().set_content(rect);
        }
    }
}

#[test]
fn a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view() {
    let mut session = EditorSession::beui::<SidebarApp>(
        Rc::new(Vec::new()),
        EditorInstanceId(0),
        Waker::default(),
    );
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    session.connect(client, Uuid::new_v4());
    session.regions.insert(
        EditorRegion::Frame,
        RegionState {
            placement: Some(ScreenPlacement {
                screen: block_plugin_api::ScreenId(0),
                instance: EditorInstanceId(0),
                region: EditorRegion::Frame,
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                scale_factor_millis: 1000,
            }),
            metrics: Some(ViewportMetrics {
                logical_width: 800.0,
                logical_height: 600.0,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
            }),
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content: Some(ChildRect {
                    x: 300.0,
                    y: 250.0,
                    width: 120.0,
                    height: 90.0,
                }),
                trail: vec!["Canvas".to_owned(), "Pan and Zoom".to_owned()],
            }),
            ..Default::default()
        },
    );

    session.run_beui(EditorRegion::Frame, 1);
    session.run_beui(EditorRegion::Frame, 2);

    let report = session
        .regions
        .get(&EditorRegion::Frame)
        .and_then(|state| state.report.as_ref())
        .expect("the frame region reports its content rect");

    assert!(
        report.content.width < 750.0,
        "expected the reported content to be narrowed to the canvas, excluding the 100px sidebar, got {:?}",
        report.content
    );
    assert!(
        report.content.x >= 99.0,
        "expected the reported content to start after the sidebar, got {:?}",
        report.content
    );
}
