use std::pin::pin;
use std::task::{Context, Poll, Waker};

mod a_child_placed_in_a_layer_names_it;
mod a_layer_is_committed_with_its_surface_in_the_frame_that_opens_it;
mod a_layers_surface_is_kept_while_it_is_open_and_released_once_it_closes;
mod a_region_without_a_layer_space_opens_no_layers;
mod a_rejected_shader_is_caught_by_an_error_scope;
mod a_surface_the_host_does_not_render_runs_its_frames_without_a_gpu;
mod dismissed_layers_are_heard_before_the_batchs_events;

use std::cell::RefCell;
use std::rc::Rc;

use block_plugin_api::{
    ChildContent, ChildLayer, ChildMode, EditorInstanceId, EditorRegion, FrameCommit,
    HelloAccepted, HostTransform, Layer, LayerId, LayerParent, LayerPlacement, LayerSpace,
    OpenBlock, Opening, ScreenId, ScreenRequest, Size, SurfaceFormat, SurfaceSpec, TimedInput,
    TopBar, ViewportMetrics,
};
use geometry::{Rect, pos2, vec2};
use uuid::Uuid;

use crate::editor_session::EditorSession;
use crate::screens::Screens;
use crate::wasm::Surfaces;
use crate::wire::Delivery;
use crate::{EditorHost, Frame, Instance, Region, RegionId, Regions};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const SCREEN: ScreenId = ScreenId(1);

#[derive(Default)]
struct Seen {
    spaces: Vec<Option<LayerSpace>>,
    painted: Vec<(LayerId, [u32; 2])>,
    heard: Vec<String>,
}

#[derive(Default)]
struct Shared {
    seen: RefCell<Seen>,
    open: RefCell<Vec<Layer>>,
    child_in: RefCell<Option<LayerId>>,
}

struct Layered {
    host: EditorHost,
    shared: Rc<Shared>,
}

impl Instance for Layered {
    fn connect(&mut self, _block_id: Uuid) {}

    fn open_region(&mut self, _region: EditorRegion) {
        if !self.host.handles_regions() {
            self.host.handle_regions(Box::new(LayeredRegions {
                host: self.host.clone(),
                shared: Rc::clone(&self.shared),
            }));
        }
    }
}

struct LayeredRegions {
    host: EditorHost,
    shared: Rc<Shared>,
}

impl Regions for LayeredRegions {
    fn input(&mut self, _region: &Region, event: &TimedInput, _over: Option<RegionId>) {
        let heard = format!("input on {:?}", event.layer);
        self.shared.seen.borrow_mut().heard.push(heard);
    }

    fn dismiss_layers(&mut self, _region: &Region, layers: &[LayerId]) {
        let heard = format!("dismissed {layers:?}");
        self.shared.seen.borrow_mut().heard.push(heard);
    }

    fn frame(&mut self, regions: &[Region]) -> Vec<Frame> {
        regions
            .iter()
            .map(|region| {
                self.shared
                    .seen
                    .borrow_mut()
                    .spaces
                    .push(region.layers.clone());
                if let Some(layer) = *self.shared.child_in.borrow() {
                    self.host.in_layer(Some(layer), || {
                        let rect = Rect::from_min_size(pos2(40.0, 50.0), vec2(30.0, 20.0));
                        self.host.place_child(
                            ChildContent::Rename([9; 16]),
                            rect,
                            rect,
                            ChildMode::Active,
                            ChildLayer::Above,
                            false,
                            TopBar::Hidden,
                            0.0,
                            1.0,
                            None,
                            false,
                            None,
                            None,
                            false,
                        )
                    });
                }
                Frame {
                    changed: true,
                    painted: vec![region.rect],
                    layers: self.shared.open.borrow().clone(),
                    ..Frame::default()
                }
            })
            .collect()
    }

    fn paint(
        &mut self,
        _region: RegionId,
        target: &crate::PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        target.whole()
    }

    fn paint_layer(
        &mut self,
        _region: RegionId,
        layer: LayerId,
        target: &crate::PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        let size = [target.width, target.height];
        self.shared.seen.borrow_mut().painted.push((layer, size));
        target.whole()
    }
}

fn space() -> LayerSpace {
    LayerSpace {
        to_host: HostTransform {
            origin: pos2(200.0, 100.0),
            scale: vec2(2.0, 1.0),
        },
        bounds: Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0)),
    }
}

fn layer(id: u64, parent: LayerParent, rect: Rect) -> Layer {
    Layer {
        id: LayerId(id),
        parent,
        placement: LayerPlacement::Anchored(Rect::from_min_size(
            pos2(10.0, 10.0),
            vec2(20.0, 20.0),
        )),
        rect,
        surface: 0,
        painted: vec![rect],
        floating: vec![rect],
        scrims: Vec::new(),
        dismiss_on_outside_press: true,
        keyboard: false,
        blocks_owner: false,
    }
}

struct Rig {
    screens: Screens,
    surfaces: Surfaces,
    requests: Vec<ScreenRequest>,
    shared: Rc<Shared>,
}

impl Rig {
    fn new(layers: Option<LayerSpace>) -> Self {
        let spec = SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            render: true,
        };
        let mut screens = Screens::new(Vec::new(), crate::Waker::default());
        screens.deliver(Delivery::Accepted(HelloAccepted {
            host_name: "test host".into(),
            surface: Some(spec),
            preferences: block_plugin_api::Preferences::default(),
        }));
        let host = EditorHost::new(crate::Waker::default());
        let shared = Rc::new(Shared::default());
        let app = Layered {
            host: host.clone(),
            shared: Rc::clone(&shared),
        };
        screens.adopt(
            INSTANCE,
            EditorSession::adopt(INSTANCE, Box::new(app), host),
        );
        let requests = vec![ScreenRequest {
            screen: SCREEN,
            instance: INSTANCE,
            region: EditorRegion::Frame,
            frame: None,
            metrics: ViewportMetrics {
                logical: Size {
                    width: 100.0,
                    height: 80.0,
                },
                visible: block_plugin_api::Pos2::ZERO,
                pixel_width: 200,
                pixel_height: 160,
                scale_factor: 2.0,
                raster: 2.0,
            },
            windows: false,
            placement: None,
            docked: false,
            layers,
        }];
        screens.deliver(Delivery::Open(Opening::Block(OpenBlock {
            instance: INSTANCE,
            editor: [6; 16],
            block_id: [1; 16],
            block_type: [2; 16],
            view_block: None,
            account_id: [3; 16],
            organization_id: [4; 16],
            client_id: [5; 16],
            editable: true,
        })));
        let surfaces = Surfaces::new(spec).expect("the test runner has a gpu");
        Self {
            screens,
            surfaces,
            requests,
            shared,
        }
    }

    fn open(&self, layers: Vec<Layer>) {
        *self.shared.open.borrow_mut() = layers;
    }

    fn draw(&mut self) -> FrameCommit {
        self.screens
            .deliver(Delivery::Screens(self.requests.clone()));
        self.screens.deliver(Delivery::Configure(1));
        self.surfaces.place(self.screens.layout().clone());
        self.surfaces
            .render(&mut self.screens)
            .expect("the frame runs")
            .expect("the plugin drew a frame")
    }

    fn screen_surface(&self) -> u32 {
        self.screens.layout().screens[0].surface
    }

    fn layers(commit: &FrameCommit) -> Vec<Layer> {
        commit
            .reports
            .iter()
            .find(|report| report.screen == SCREEN)
            .map(|report| report.layers.clone())
            .unwrap_or_default()
    }
}
