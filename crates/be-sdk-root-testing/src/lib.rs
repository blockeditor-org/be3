//! A root guest built on be-sdk-root, which be-wasm-root-host's tests run
//! (plans/host_as_platform_adapter.md, step 4e5b). On its first frame it spawns the triangle as
//! a staged app (`APP`) and opens its editor with one screen (`SCREEN`), painted at `RASTER`
//! pixels per point; each frame it asks the app for a frame when one is due, races that against
//! the frame's deadline, and shows its own surface with the triangle's screen over it at
//! `ORIGIN`, by handle, once the app painted it, and `LATE_LAYER` when the deadline came first.
//! Its own surface (`ROOT_SURFACE`) it draws itself, on the host's device, filled with
//! `ROOT_COLOR` on its first frame.
//! A frame at `CLOSES` lets go of the app, which stops it. The window's input is routed by the
//! library over the layers it drew last.
//!
//! The triangle needs no block content (its `connect` reads nothing), so the root serves it no
//! block store: that waits for step 5.

use std::{
    cell::RefCell,
    future::Future,
    pin::{Pin, pin},
    rc::Rc,
    task::Poll,
    time::Duration,
};

use be_lib_geometry::pos2;
use be_lib_layers::{Layer, LayerId, LayerList, Surface, WindowLayers};
use be_sdk_api::{
    EditorInstanceId, OpenBlock, Opening, OpeningKind, ParentView, Pos2 as ViewPos2, ScreenId,
    ScreenRequest, SurfaceFormat, SurfaceSpec, Vec2 as ViewVec2, ViewId, ViewPlacement,
    ViewportMetrics,
};
use be_sdk_root::{Embedded, Embedding, OwnSurface, Spawn};
use be_wasm_guest_abi::{Flow, Guest, Waker};
use be_wasm_ops::guest::{Executor, LocalFuture};
use be_wasm_root_api::{
    FrameDone, FrameRequest,
    guest::{
        exports::{frames, input},
        host,
    },
};

/// The staged app it spawns: the triangle's, by its manifest's id.
pub const APP: &str = "be3.triangle";

/// The triangle's entry point, as its manifest names it.
pub const APP_ENTRY: &str = "triangle.wasm";

/// The triangle's editor, by its manifest's id.
const EDITOR: [u8; 16] = [
    0x52, 0x54, 0xd0, 0xa5, 0xae, 0x87, 0x4a, 0xfe, 0x84, 0x58, 0x50, 0xfd, 0xe9, 0xcc, 0x0d, 0x2b,
];

/// The triangle's block type, by its manifest's id.
const BLOCK_TYPE: [u8; 16] = [
    0x77, 0x67, 0x70, 0x75, 0x2d, 0x74, 0x72, 0x69, 0x61, 0x6e, 0x67, 0x6c, 0x65, 0x30, 0x30, 0x31,
];

/// The one screen the root asks of the editor.
pub const SCREEN: ScreenId = ScreenId(1);

/// The screen's size, in points.
pub const SCREEN_SIZE: (u32, u32) = (320, 200);

/// The pixels per point the screen is laid out and painted at.
pub const RASTER: u32 = 2;

/// Where the screen's top left sits in the window.
pub const ORIGIN: (f32, f32) = (40.0, 20.0);

/// The window's size, in points.
pub const SIZE: (u32, u32) = (400, 240);

/// The root's own surface, under the screen.
pub const ROOT_LAYER: LayerId = LayerId(0);

/// The id of the root's own surface, which `ROOT_LAYER` shows: it fills the window, a pixel to
/// a point.
pub const ROOT_SURFACE: u32 = 0;

/// What the root fills its own surface with, sRGB-encoded: a dull purple, unlike any window's
/// background or the triangle's colours.
pub const ROOT_COLOR: [f64; 4] = [0.3, 0.1, 0.4, 1.0];

/// Shown over the rest when the frame's deadline passed before the app answered.
pub const LATE_LAYER: LayerId = LayerId(1);

/// A frame at this time lets go of the app, which stops it, and shows the root's surface alone.
pub const CLOSES: u64 = u64::MAX;

#[derive(Clone)]
struct Root {
    embedding: Rc<RefCell<Embedding>>,
    app: Rc<RefCell<Option<Embedded>>>,
    /// Its own surface, made and filled on its first frame.
    own: Rc<RefCell<Option<OwnSurface>>>,
}

impl Default for Root {
    fn default() -> Self {
        Self {
            embedding: Rc::new(RefCell::new(Embedding::new(ROOT_LAYER))),
            app: Rc::default(),
            own: Rc::default(),
        }
    }
}

impl Root {
    /// The layer showing its own surface, which it fills the first time.
    fn own_layer(&self) -> Result<Layer, String> {
        let mut own = self.own.borrow_mut();
        let own = match &mut *own {
            Some(own) => own,
            None => {
                let made = OwnSurface::new(ROOT_SURFACE, SIZE);
                made.fill(ROOT_COLOR)?;
                own.insert(made)
            }
        };
        Ok(own.layer(ROOT_LAYER, pos2(0.0, 0.0), 1.0))
    }

    /// The triangle, spawned the first time.
    fn app(&self, now: Duration) -> Embedded {
        let mut app = self.app.borrow_mut();
        let app = app.get_or_insert_with(|| {
            let app = Embedded::spawn(
                Spawn {
                    app: APP.to_owned(),
                    module: APP_ENTRY.to_owned(),
                    opening: opening(),
                    screens: vec![screen()],
                    surface: SurfaceSpec {
                        format: SurfaceFormat::Rgba8Unorm,
                        render: true,
                    },
                },
                now,
            );
            self.embedding.borrow_mut().embed(app.clone());
            app
        });
        app.clone()
    }
}

impl frames::Guest for Root {
    fn frame(&self, request: FrameRequest) -> LocalFuture<FrameDone> {
        let root = self.clone();
        Box::pin(async move {
            let own = root
                .own_layer()
                .expect("the root's host gives it a device to draw its own surface on");
            let mut layers = vec![own.clone()];
            if request.time == CLOSES {
                if let Some(app) = root.app.borrow_mut().take() {
                    root.embedding.borrow_mut().remove(app.instance());
                }
            } else {
                let now = Duration::from_nanos(request.time);
                let app = root.app(now);
                let deadline = pin!(host::deadline());
                let drawn = pin!(app.frame(now));
                let late = !first(drawn, deadline).await;
                layers.extend(app.layer(SCREEN, pos2(ORIGIN.0, ORIGIN.1)));
                if late {
                    let mut marker = own;
                    marker.id = LATE_LAYER;
                    marker.surface = Surface::Root(1);
                    layers.push(marker);
                }
            }
            let list = LayerList {
                windows: vec![WindowLayers {
                    window: None,
                    layers,
                }],
            };
            let mut embedding = root.embedding.borrow_mut();
            let wants = embedding.wants();
            let layers = embedding.drew(list).layers;
            let requests = embedding.requests(None);
            FrameDone {
                layers,
                next: wants
                    .next
                    .map(|at| u64::try_from(at.as_nanos()).unwrap_or(u64::MAX)),
                owed: wants.owed,
                ime: embedding.ime(None).map(|area| area.encode()),
                every_sample: requests.every_sample,
                cursor: requests.cursor.name().to_owned(),
                pointer_locked: requests.pointer_locked,
            }
        })
    }
}

impl input::Guest for Root {
    fn deliver(&self, window: Option<u64>, events: Vec<u8>) -> LocalFuture<()> {
        // What lands on the root's own surface it does nothing with: it draws no UI of its own.
        let _own = self.embedding.borrow_mut().input(window, &events);
        Box::pin(async {})
    }
}

/// Waits for whichever of `left` and `right` finishes first: answers whether `left` did.
async fn first<A: Future, B: Future>(mut left: Pin<&mut A>, mut right: Pin<&mut B>) -> bool {
    std::future::poll_fn(|context| {
        if left.as_mut().poll(context).is_ready() {
            return Poll::Ready(true);
        }
        match right.as_mut().poll(context).is_ready() {
            true => Poll::Ready(false),
            false => Poll::Pending,
        }
    })
    .await
}

fn opening() -> Opening {
    Opening {
        instance: EditorInstanceId(1),
        editor: EDITOR,
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        kind: OpeningKind::Block(OpenBlock {
            block_id: [1; 16],
            block_type: BLOCK_TYPE,
            view_block: None,
            editable: true,
            page_origin: String::new(),
            host_palette: false,
        }),
    }
}

fn screen() -> ScreenRequest {
    let (width, height) = SCREEN_SIZE;
    ScreenRequest {
        screen: SCREEN,
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: ViewVec2 {
                x: width as f32,
                y: height as f32,
            },
            visible: ViewPos2::ZERO,
            pixel_width: width * RASTER,
            pixel_height: height * RASTER,
            scale_factor: RASTER as f32,
            raster: RASTER as f32,
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
        fit: false,
        origin: None,
    }
}

pub struct Embedder {
    executor: Executor,
}

impl Guest for Embedder {
    fn start(_: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            executor: be_wasm_root_api::guest::executor(Root::default()),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.executor.step(inbound, outbound)?;
        Ok(Flow::Running)
    }
}

// Only the module exports it, so a crate that reads the constants above (be-wasm-root-host's
// tests) links no guest's entry points.
#[cfg(feature = "module")]
be_wasm_guest_abi::guest!(Embedder);
