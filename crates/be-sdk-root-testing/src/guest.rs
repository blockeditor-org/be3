//! The root itself, built for the apps' platform alone.

use std::{
    cell::RefCell,
    future::Future,
    pin::{Pin, pin},
    rc::Rc,
    task::Poll,
    time::Duration,
};

use be_lib_geometry::pos2;
use be_lib_layers::{LayerList, Surface, WindowLayers};
use be_sdk_api::{
    EditorInstanceId, OpenBlock, Opening, OpeningKind, ParentView, Pos2 as ViewPos2,
    ScreenRequest, SurfaceFormat, SurfaceSpec, Vec2 as ViewVec2, ViewId, ViewPlacement,
    ViewportMetrics,
};
use be_sdk_beui_root::BeuiSurface;
use be_sdk_root::{Delivery, Embedded, Embedding, OwnSurface, Spawn};
use be_wasm_guest_abi::{Flow, Guest, Waker};
use be_wasm_ops::guest::{Executor, LocalFuture};
use be_wasm_root_api::{
    FrameDone, FrameRequest,
    guest::{
        exports::{frames, input, paintings},
        host,
    },
};
use beui::native::painting::ScreenPaintings;
use beui::primitives::{Frame, Text};
use beui::reactive::*;
use beui::{Align2, Color32, FontId, Sides};

use crate::{
    APP, APP_ENTRY, CLOSES, LATE_LAYER, ORIGIN, RASTER, RESIZED, RESIZES, ROOT_COLOR, ROOT_LAYER,
    ROOT_SURFACE, SCREEN, SCREEN_SIZE, SIZE, TEXT, TEXT_ORIGIN, TEXT_SIZE, UNPRESENTED_SURFACE,
};

/// The triangle's editor, by its manifest's id.
const EDITOR: [u8; 16] = [
    0x52, 0x54, 0xd0, 0xa5, 0xae, 0x87, 0x4a, 0xfe, 0x84, 0x58, 0x50, 0xfd, 0xe9, 0xcc, 0x0d, 0x2b,
];

/// The triangle's block type, by its manifest's id.
const BLOCK_TYPE: [u8; 16] = [
    0x77, 0x67, 0x70, 0x75, 0x2d, 0x74, 0x72, 0x69, 0x61, 0x6e, 0x67, 0x6c, 0x65, 0x30, 0x30, 0x31,
];

#[derive(Clone)]
struct Root {
    embedding: Rc<RefCell<Embedding>>,
    app: Rc<RefCell<Option<Embedded>>>,
    /// Its own surface, which beui draws, made on its first frame.
    own: Rc<RefCell<Option<BeuiSurface>>>,
    /// The surface a `RESIZES` frame configures and never presents.
    unpresented: Rc<RefCell<Option<OwnSurface>>>,
    /// Wakes the root, and so its host, for a frame its beui document wants.
    waker: Waker,
}

impl Root {
    fn new(waker: Waker) -> Self {
        Self {
            embedding: Rc::new(RefCell::new(Embedding::new(ROOT_LAYER))),
            app: Rc::default(),
            own: Rc::default(),
            unpresented: Rc::default(),
            waker,
        }
    }

    /// Its own surface, made the first time with the host's fonts.
    async fn own(&self) -> Result<(), String> {
        if self.own.borrow().is_some() {
            return Ok(());
        }
        let fonts = host::fonts()
            .await
            .map_err(|error| format!("the host gave no fonts: {error:?}"))?;
        let waker = self.waker.clone();
        let surface = BeuiSurface::new(
            OwnSurface::new(ROOT_SURFACE, SIZE),
            1.0,
            &fonts,
            move || waker.wake(),
            document,
        )?;
        *self.own.borrow_mut() = Some(surface);
        Ok(())
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

    /// Hands its beui document what landed on its own surface.
    fn heard(&self, own: &[Delivery]) {
        if let Some(surface) = &mut *self.own.borrow_mut() {
            surface.input(own);
        }
    }
}

/// What the root's own surface shows: `ROOT_COLOR` everywhere, and `TEXT` at `TEXT_ORIGIN`.
fn document() -> beui::primitives::Root {
    let [red, green, blue, _] = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    let (left, top) = TEXT_ORIGIN;
    view! {
        <beui::primitives::Root clear_color={Color32::from_rgb(red, green, blue)}>
            <Frame align=Align2::START padding={Sides::new(left, top, 0.0, 0.0)}>
                <Text font={FontId::proportional(TEXT_SIZE)} color={Color32::WHITE}>{TEXT}</Text>
            </Frame>
        </beui::primitives::Root>
    }
}

impl frames::Guest for Root {
    fn frame(&self, request: FrameRequest) -> LocalFuture<FrameDone> {
        let root = self.clone();
        Box::pin(async move {
            root.own()
                .await
                .expect("the root's host gives it fonts and a device to draw its own surface on");
            if request.time == RESIZES {
                if let Some(own) = &mut *root.own.borrow_mut() {
                    own.resize(RESIZED, 1.0);
                }
                *root.unpresented.borrow_mut() = Some(OwnSurface::new(UNPRESENTED_SURFACE, SIZE));
            } else if let Some(own) = &mut *root.own.borrow_mut()
                && own.wants_frame()
            {
                own.frame().expect("the root draws its own surface with beui");
            }
            let own = root
                .own
                .borrow()
                .as_ref()
                .map(|own| own.layer(ROOT_LAYER, pos2(0.0, 0.0)))
                .expect("the root made its own surface");
            let mut layers = vec![own.clone()];
            if request.time == CLOSES {
                if let Some(app) = root.app.borrow_mut().take() {
                    root.embedding.borrow_mut().remove(app.instance());
                }
            } else if request.time != RESIZES {
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
            let drawn = embedding.drew(list);
            root.heard(&drawn.own);
            let requests = embedding.requests(None);
            // Its document wants a frame at once (it heard input) or after a while.
            let document = root.own.borrow().as_ref().and_then(BeuiSurface::wait);
            let time = Duration::from_nanos(request.time);
            let next = match (wants.next, document.map(|wait| time + wait)) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (left, right) => left.or(right),
            };
            FrameDone {
                layers: drawn.layers,
                next: next.map(|at| u64::try_from(at.as_nanos()).unwrap_or(u64::MAX)),
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
        let own = self
            .embedding
            .borrow_mut()
            .input(window, &events)
            .unwrap_or_default();
        self.heard(&own);
        Box::pin(async {})
    }
}

impl paintings::Guest for Root {
    fn paintings(&self) -> LocalFuture<Vec<u8>> {
        let painted = self.own.borrow().as_ref().and_then(|own| {
            Some(ScreenPaintings {
                screens: vec![(u64::from(own.id()), own.painting()?)],
                overflows: Vec::new(),
            })
        });
        Box::pin(async move { painted.map(|painted| painted.encode()).unwrap_or_default() })
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
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            executor: be_wasm_root_api::guest::executor(Root::new(waker)),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        // What the gpu settled since the last step, as be-sdk polls for an app.
        be_wasm_gpu_guest::poll();
        self.executor.step(inbound, outbound)?;
        Ok(Flow::Running)
    }
}

be_wasm_guest_abi::guest!(Embedder);
