use std::pin::pin;
use std::task::{Context, Poll, Waker};

mod a_child_placed_in_an_anchored_view_is_placed_in_its_own_points;
mod a_rejected_shader_is_caught_by_an_error_scope;
mod a_surface_the_host_does_not_render_runs_its_frames_without_a_gpu;
mod a_view_without_an_anchor_space_anchors_no_views;
mod a_window_is_committed_with_its_own_screen_in_the_frame_that_opens_it;
mod an_anchored_view_is_a_screen_committed_in_the_frame_that_opens_it;
mod an_anchored_views_surface_is_kept_while_it_is_open_and_released_once_it_closes;
mod dismissed_anchored_views_are_heard_before_the_batchs_events;

use std::cell::RefCell;
use std::rc::Rc;

use block_plugin_api::{
    AnchorSpace, AnchoredPlacement, ChildContent, ChildLayer, ChildMode, EditorInstanceId,
    FrameCommit, HelloAccepted, HostTransform, OpenBlock, Opening, ParentView, ScreenId,
    ScreenRequest, Size, SurfaceFormat, SurfaceSpec, TimedInput, TopBar, View, ViewAnchor, ViewId,
    ViewPlacement, ViewRef, ViewportMetrics,
};
use geometry::{Rect, pos2, vec2};
use uuid::Uuid;

use crate::editor_session::EditorSession;
use crate::screens::Screens;
use crate::wasm::Surfaces;
use crate::wire::Delivery;
use crate::{Anchored, EditorHost, Frame, Instance, ShownView, ViewHandler};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const SCREEN: ScreenId = ScreenId(1);

#[derive(Default)]
struct Seen {
    spaces: Vec<Option<AnchorSpace>>,
    painted: Vec<(ViewId, [u32; 2])>,
    heard: Vec<String>,
}

#[derive(Default)]
struct Shared {
    seen: RefCell<Seen>,
    open: RefCell<Vec<Anchored>>,
    child_in: RefCell<Option<ViewId>>,
    window: RefCell<Option<block_plugin_api::WindowOpen>>,
}

struct Layered {
    host: EditorHost,
    shared: Rc<Shared>,
}

impl Instance for Layered {
    fn connect(&mut self, _block_id: Uuid) {}

    fn open_view(&mut self, _view: View) {
        if !self.host.handles_views() {
            self.host.handle_views(Box::new(AnchoringViews {
                host: self.host.clone(),
                shared: Rc::clone(&self.shared),
            }));
        }
    }
}

struct AnchoringViews {
    host: EditorHost,
    shared: Rc<Shared>,
}

impl ViewHandler for AnchoringViews {
    fn input(&mut self, _region: &ShownView, event: &TimedInput, _over: Option<ViewRef>) {
        let heard = format!("input on {:?} at {:?}", _region.view.id, event.input);
        self.shared.seen.borrow_mut().heard.push(heard);
    }

    fn dismiss_anchored(&mut self, _region: &ShownView, views: &[ViewId]) {
        let heard = format!("dismissed {views:?}");
        self.shared.seen.borrow_mut().heard.push(heard);
    }

    fn frame(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        regions
            .iter()
            .map(|region| {
                self.shared
                    .seen
                    .borrow_mut()
                    .spaces
                    .push(region.anchor_space.clone());
                if let Some(layer) = *self.shared.child_in.borrow() {
                    self.host.in_view(View::anchored(layer), || {
                        let rect = Rect::from_min_size(pos2(230.0, 140.0), vec2(30.0, 20.0));
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
                            0.0,
                            None,
                            false,
                            None,
                            None,
                            false,
                            None,
                        )
                    });
                }
                Frame {
                    changed: true,
                    painted: vec![region.rect],
                    anchored: self.shared.open.borrow().clone(),
                    asks: self
                        .shared
                        .window
                        .borrow_mut()
                        .take()
                        .map(block_plugin_api::WindowRequest::Open)
                        .into_iter()
                        .collect(),
                    ..Frame::default()
                }
            })
            .collect()
    }

    fn frame_opened(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        regions
            .iter()
            .map(|region| Frame {
                painted: vec![region.rect],
                floating: vec![region.rect],
                ..Frame::default()
            })
            .collect()
    }

    fn paint(
        &mut self,
        region: ViewRef,
        target: &crate::PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        if self
            .shared
            .open
            .borrow()
            .iter()
            .any(|open| open.view == region.view)
        {
            let size = [target.width, target.height];
            self.shared
                .seen
                .borrow_mut()
                .painted
                .push((region.view, size));
        }
        target.whole()
    }
}

fn space() -> AnchorSpace {
    AnchorSpace {
        to_host: HostTransform {
            origin: pos2(200.0, 100.0),
            scale: vec2(2.0, 1.0),
        },
        bounds: Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0)),
    }
}

fn layer(id: u64, parent: u64, rect: Rect) -> Anchored {
    Anchored {
        view: ViewId(id),
        placement: AnchoredPlacement {
            parent: ViewId(parent),
            anchor: ViewAnchor::Rect(Rect::from_min_size(pos2(10.0, 10.0), vec2(20.0, 20.0))),
            rect,
            dismiss_on_outside_press: true,
            trigger: None,
            keyboard: false,
            blocks_owner: false,
        },
    }
}

struct Rig {
    screens: Screens,
    surfaces: Surfaces,
    requests: Vec<ScreenRequest>,
    shared: Rc<Shared>,
}

impl Rig {
    fn new(layers: Option<AnchorSpace>) -> Self {
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
            view: ViewId::MAIN,
            placement: ViewPlacement::Parent(ParentView::Main),
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
            host_place: None,
            origin: None,
            docked: false,
            anchor_space: layers,
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

    fn open(&self, layers: Vec<Anchored>) {
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
        self.placed(SCREEN).surface
    }

    fn placed(&self, screen: ScreenId) -> block_plugin_api::ScreenPlacement {
        *self
            .screens
            .layout()
            .placement(screen)
            .expect("the screen is laid out")
    }

    fn anchored(commit: &FrameCommit) -> Vec<(ScreenId, AnchoredPlacement)> {
        commit
            .reports
            .iter()
            .filter_map(|report| Some((report.screen, report.anchored?)))
            .collect()
    }

    fn anchored_screens(&self) -> Vec<(ViewId, u32)> {
        self.screens
            .layout()
            .screens
            .iter()
            .filter(|placement| placement.screen.is_opened())
            .map(|placement| (placement.view, placement.surface))
            .collect()
    }
}
