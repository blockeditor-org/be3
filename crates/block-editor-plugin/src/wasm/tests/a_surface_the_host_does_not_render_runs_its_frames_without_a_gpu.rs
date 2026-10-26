use block_plugin_api::{
    EditorInstanceId, EditorRegion, HelloAccepted, OpenBlock, Opening, ScreenId, ScreenRequest,
    Size, SurfaceFormat, SurfaceSpec, TimedInput, ViewportMetrics,
};
use uuid::Uuid;

use crate::editor_session::EditorSession;
use crate::screens::Screens;
use crate::wasm::{Surfaces, surface_gpu};
use crate::wire::Delivery;
use crate::{EditorHost, Frame, Instance, Region, RegionId, Regions, Waker};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);

struct Unpainted(EditorHost);

impl Instance for Unpainted {
    fn connect(&mut self, _block_id: Uuid) {}

    fn open_region(&mut self, _region: EditorRegion) {
        if !self.0.handles_regions() {
            self.0.handle_regions(Box::new(UnpaintedRegions));
        }
    }
}

struct UnpaintedRegions;

impl Regions for UnpaintedRegions {
    fn input(&mut self, _region: &Region, _event: &TimedInput, _over: Option<RegionId>) {}

    fn frame(&mut self, regions: &[Region]) -> Vec<Frame> {
        regions
            .iter()
            .map(|region| Frame {
                changed: true,
                painted: vec![region.rect],
                ..Frame::default()
            })
            .collect()
    }

    fn paint(
        &mut self,
        _region: crate::RegionId,
        _target: &crate::PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        panic!("a surface the host does not render was painted")
    }
}

#[test]
fn a_surface_the_host_does_not_render_runs_its_frames_without_a_gpu() {
    let spec = SurfaceSpec {
        format: SurfaceFormat::Rgba8Unorm,
        render: false,
    };
    let mut screens = Screens::new(Vec::new(), Waker::default());
    screens.deliver(Delivery::Accepted(HelloAccepted {
        host_name: "test host".into(),
        surface: Some(spec),
        preferences: block_plugin_api::Preferences::default(),
    }));
    let host = EditorHost::new(Waker::default());
    screens.adopt(
        INSTANCE,
        EditorSession::adopt(INSTANCE, Box::new(Unpainted(host.clone())), host),
    );
    for delivery in [
        Delivery::Open(Opening::Block(OpenBlock {
            instance: INSTANCE,
            editor: [6; 16],
            block_id: [1; 16],
            block_type: [2; 16],
            view_block: None,
            account_id: [3; 16],
            organization_id: [4; 16],
            client_id: [5; 16],
            editable: true,
        })),
        Delivery::Screens(vec![ScreenRequest {
            screen: ScreenId(1),
            instance: INSTANCE,
            region: EditorRegion::Frame,
            frame: None,
            metrics: ViewportMetrics {
                logical: Size {
                    width: 100.0,
                    height: 80.0,
                },
                visible: block_plugin_api::Pos2::ZERO,
                pixel_width: 100,
                pixel_height: 80,
                scale_factor: 1.0,
                raster: 1.0,
            },
            windows: false,
            placement: None,
        }]),
        Delivery::Configure(1),
    ] {
        screens.deliver(delivery);
    }

    let mut surfaces = Surfaces::new(spec).expect("surfaces need no gpu when not rendered");
    surfaces.place(screens.layout().clone());
    let commit = surfaces
        .render(&mut screens)
        .expect("the frame runs")
        .expect("the plugin drew one frame");
    assert_eq!(commit.reports.len(), 1, "the editor's frame ran");
    assert_eq!(commit.presented, None, "nothing was presented");
    assert!(surface_gpu().is_none(), "no gpu device was opened");
}
