//! The root guest be-wasm-root-host's tests run (plans/host_as_platform_adapter.md, The root
//! guest): it settles one child as the compositor settles its apps. On its first frame it spawns
//! the echo guest as a staged app (`CHILD`); each frame it asks the child to echo the frame's
//! time, and races that whole exchange against the frame's deadline. Its layer list shows its own
//! surface, and over it the child's screen only when the child echoed in time.
//!
//! It routes the window's input (`input.deliver`) as the compositor will, with be-sdk-root's
//! `Windows` (a `Router` per window over the layers it drew last), and answers once it has: what
//! lands on the child's screen goes to the child with the next frame, in the child's own
//! coordinates, and the child echoes it back. A frame then shows `HEARD_LAYER`
//! at the last point the child heard.
//!
//! A frame at `PAINTS` instead spawns a child that paints (`PAINTER`), asks it for a frame, and
//! shows the surface it presented, named only by the instance's handle (its `instance` resource's
//! op) and the surface's id: the root never sees the pixels, which stay on the host's device.

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::{Pin, pin},
    rc::Rc,
    task::Poll,
};

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerId, LayerInput, LayerList, Surface, WindowLayers};
use be_lib_ui_types::input::{self as ui_input, Input};
use be_sdk_root::Windows;
use be_wasm_guest_abi::{Flow, Guest, Waker};
use be_wasm_ops::guest::{Executor, LocalFuture};
use be_wasm_root_api::{
    FrameDone, FrameRequest,
    guest::{
        exports::{frames, input},
        host,
    },
};
use be_wasm_sandbox_api::{
    guest::sandbox::Instance,
    sandbox::{Config, Source, StagedModule},
};

/// The staged app it spawns: the echo guest's (be_wasm_guest_echo::APP_ID).
pub const CHILD: &str = "be3.echo";

/// The child's entry point, as its manifest names it.
pub const CHILD_ENTRY: &str = "echo.wasm";

/// A frame at this time first asks the child to hang (be_wasm_guest_echo::HANG), so the child
/// never echoes and the frame ends at its deadline.
pub const HANGS_CHILD: u64 = u64::MAX;

/// A frame at this time never answers, and the root wakes itself on every step from then on, as a
/// root stuck in a busy loop of its own would.
pub const SPINS: u64 = u64::MAX - 1;

const HANG: &[u8] = b"hang";

/// A frame at this time spawns `PAINTER` rather than settling the echo guest, and shows the
/// surface it presents (`PAINTED_SURFACE`) as `PAINTER_LAYER`.
pub const PAINTS: u64 = u64::MAX - 2;

/// The staged app a `PAINTS` frame spawns, which paints a surface the first time it is stepped
/// and presents it: the tests' loader stages one under this id.
pub const PAINTER: &str = "be3.painter";

/// The painter's entry point, as its loader names it.
pub const PAINTER_ENTRY: &str = "painter.wasm";

/// The painter's surface, which it presents.
pub const PAINTED_SURFACE: u32 = 7;

/// The painter's surface, as a `PAINTS` frame shows it.
pub const PAINTER_LAYER: LayerId = LayerId(2);

/// The root's own surface, which every frame shows.
pub const ROOT_LAYER: LayerId = LayerId(0);

/// The child's screen, which a frame shows when the child echoed in time.
pub const CHILD_LAYER: LayerId = LayerId(1);

/// Marks the last point the child heard input at, in the child's own coordinates: its
/// transform's origin is that point. A frame shows it once the child heard any.
pub const HEARD_LAYER: LayerId = LayerId(3);

/// The window's size, in points.
pub const SIZE: (u32, u32) = (200, 100);

/// Where the child's screen sits in the window: its right half.
pub const CHILD_ORIGIN: (f32, f32) = (SIZE.0 as f32 / 2.0, 0.0);

/// What starts a frame the root sends the child with input routed to it; the rest is the
/// input, in postcard.
const ROUTED: &[u8] = b"routed:";

#[derive(Clone)]
struct Frames {
    child: Rc<RefCell<Option<Rc<Instance>>>>,
    painter: Rc<RefCell<Option<Rc<Instance>>>>,
    spinning: Rc<Cell<bool>>,
    /// A router per window over the layers the root drew last: who holds the pointer, the hover
    /// and the focus.
    windows: Rc<RefCell<Windows>>,
    /// The last point the child heard input at, in its own coordinates.
    heard: Rc<Cell<Option<Pos2>>>,
    /// Input routed to the child, which it is sent ahead of the next frame's ask.
    routed: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl Default for Frames {
    fn default() -> Self {
        Self {
            child: Rc::default(),
            painter: Rc::default(),
            spinning: Rc::default(),
            windows: Rc::new(RefCell::new(Windows::new(ROOT_LAYER))),
            heard: Rc::default(),
            routed: Rc::default(),
        }
    }
}

impl Frames {
    fn child(&self) -> Rc<Instance> {
        let mut child = self.child.borrow_mut();
        let child = child.get_or_insert_with(|| {
            Rc::new(Instance::new(
                Source::App(StagedModule {
                    app: CHILD.to_owned(),
                    module: CHILD_ENTRY.to_owned(),
                }),
                Config {
                    name: "settled child".to_owned(),
                    // An app links its libraries into one shared memory, which the base
                    // library alone fills much of.
                    memory_limit: 1 << 30,
                },
            ))
        });
        Rc::clone(child)
    }
}

impl frames::Guest for Frames {
    fn frame(&self, request: FrameRequest) -> LocalFuture<FrameDone> {
        if request.time == SPINS {
            self.spinning.set(true);
            return Box::pin(std::future::pending());
        }
        let frames = self.clone();
        if request.time == PAINTS {
            return Box::pin(async move {
                let deadline = pin!(host::deadline());
                let painted = pin!(frames.paint());
                let painter = match first(painted, deadline).await {
                    First::Left(painter) => painter,
                    First::Right(_) => None,
                };
                frames.drew(painted_layers(painter))
            });
        }
        Box::pin(async move {
            let deadline = pin!(host::deadline());
            let exchange = pin!(frames.exchange(request.time));
            let echoed = matches!(first(exchange, deadline).await, First::Left(true));
            let mut list = layers(echoed);
            if let Some(heard) = frames.heard.get() {
                list.windows[0].layers.push(heard_layer(heard));
            }
            frames.drew(list)
        })
    }
}

impl input::Guest for Frames {
    fn deliver(&self, window: Option<u64>, events: Vec<u8>) -> LocalFuture<()> {
        let routed = match ui_input::decode(&events) {
            Ok(events) => self.route(window, events),
            Err(_) => Vec::new(),
        };
        // Answered once routed: what is for the child goes ahead of the next frame's ask.
        self.routed.borrow_mut().extend(routed);
        Box::pin(async {})
    }
}

impl Frames {
    /// Routes `events` by the layers drawn last for `window`, and answers the frames that send
    /// the child what is for it.
    fn route(&self, window: Option<u64>, events: Vec<Input>) -> Vec<Vec<u8>> {
        self.windows
            .borrow_mut()
            .route(window, events)
            .into_iter()
            .filter(|delivery| delivery.target == CHILD_LAYER)
            .map(|delivery| {
                let mut frame = ROUTED.to_vec();
                frame.extend(postcard::to_allocvec(&delivery.input).expect("input serializes"));
                frame
            })
            .collect()
    }

    /// Keeps `list` as the layers drawn last, and answers it.
    fn drew(&self, list: LayerList) -> FrameDone {
        let layers = list.encode();
        // The child hears nothing of a window that went: the root draws one window always.
        let _lost = self.windows.borrow_mut().drew(list);
        // Its host draws every frame it is asked for in its tests, so it asks for none.
        FrameDone { layers, next: None }
    }

    /// Notes where an echoed frame of input routed to the child says the child heard it.
    fn heard(&self, frame: &[u8]) {
        let Some(input) = frame
            .strip_prefix(ROUTED)
            .and_then(|input| postcard::from_bytes::<Input>(input).ok())
        else {
            return;
        };
        if let Input::PointerMoved(at) | Input::PointerButton { pos: at, .. } = input {
            self.heard.set(Some(at));
        }
    }

    /// Asks the child to echo `time`, spawning it first if need be, and answers whether it did.
    async fn exchange(&self, time: u64) -> bool {
        let child = self.child();
        if time == HANGS_CHILD {
            let _ = child.send(vec![HANG.to_vec()]).await;
        }
        let asked = time.to_le_bytes().to_vec();
        let mut sending = std::mem::take(&mut *self.routed.borrow_mut());
        sending.push(asked.clone());
        if child.send(sending).await.is_err() {
            return false;
        }
        loop {
            match child.next().await {
                Ok(Ok(frames)) => {
                    for frame in &frames {
                        self.heard(frame);
                    }
                    if frames.contains(&asked) {
                        return true;
                    }
                }
                // It stopped, or the host stopped answering for it.
                _ => return false,
            }
        }
    }
}

impl Frames {
    /// Spawns the painter, the first time, and asks it for a frame. Answers the handle of the
    /// instance that presented, its `instance` resource's op, once it answered, or None if it
    /// stopped.
    async fn paint(&self) -> Option<u64> {
        let painter = Rc::clone(self.painter.borrow_mut().get_or_insert_with(|| {
            Rc::new(Instance::new(
                Source::App(StagedModule {
                    app: PAINTER.to_owned(),
                    module: PAINTER_ENTRY.to_owned(),
                }),
                Config {
                    name: "painter".to_owned(),
                    memory_limit: 16 << 20,
                },
            ))
        }));
        painter.send(vec![vec![0]]).await.ok()?;
        match painter.next().await {
            Ok(Ok(_)) => Some(painter.op()),
            _ => None,
        }
    }
}

enum First<A, B> {
    Left(A),
    Right(B),
}

/// Whichever of `left` and `right` finishes first, `left` when both have.
async fn first<A: Future, B: Future>(
    mut left: Pin<&mut A>,
    mut right: Pin<&mut B>,
) -> First<A::Output, B::Output> {
    std::future::poll_fn(|context| {
        if let Poll::Ready(value) = left.as_mut().poll(context) {
            return Poll::Ready(First::Left(value));
        }
        if let Poll::Ready(value) = right.as_mut().poll(context) {
            return Poll::Ready(First::Right(value));
        }
        Poll::Pending
    })
    .await
}

fn layer(id: LayerId, surface: Surface, size: (u32, u32)) -> Layer {
    Layer {
        id,
        surface,
        size,
        transform: Affine::IDENTITY,
        clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

fn layers(echoed: bool) -> LayerList {
    let mut layers = vec![layer(ROOT_LAYER, Surface::Root(0), SIZE)];
    if echoed {
        let mut child = layer(
            CHILD_LAYER,
            Surface::Screen {
                instance: 0,
                surface: 0,
            },
            (SIZE.0 / 2, SIZE.1),
        );
        child.transform = Affine::from_axes(
            vec2(1.0, 0.0),
            vec2(0.0, 1.0),
            pos2(CHILD_ORIGIN.0, CHILD_ORIGIN.1),
        );
        layers.push(child);
    }
    LayerList {
        windows: vec![WindowLayers {
            window: None,
            layers,
        }],
    }
}

/// Marks `heard`, a point in the child's coordinates, with a layer of its own whose origin it is.
fn heard_layer(heard: Pos2) -> Layer {
    let mut marker = layer(HEARD_LAYER, Surface::Root(1), (1, 1));
    marker.transform = Affine::from_axes(vec2(1.0, 0.0), vec2(0.0, 1.0), heard);
    marker.input.hit = be_lib_layers::Area::Nowhere;
    marker
}

/// The root's own surface, and over it the painter's surface, by the painter's handle, once it
/// painted.
fn painted_layers(painter: Option<u64>) -> LayerList {
    let mut list = layers(false);
    if let Some(instance) = painter {
        list.windows[0].layers.push(layer(
            PAINTER_LAYER,
            Surface::Screen {
                instance,
                surface: PAINTED_SURFACE,
            },
            (4, 4),
        ));
    }
    list
}

pub struct Settling {
    executor: Executor,
    waker: Waker,
    spinning: Rc<Cell<bool>>,
}

impl Guest for Settling {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        let frames = Frames::default();
        Ok(Self {
            spinning: Rc::clone(&frames.spinning),
            executor: be_wasm_root_api::guest::executor(frames),
            waker,
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.executor.step(inbound, outbound)?;
        if self.spinning.get() {
            self.waker.wake();
        }
        Ok(Flow::Running)
    }
}

// Only the module exports it, so a crate that reads the constants above (be-wasm-root-web-testing,
// on the page) links no guest's entry points.
#[cfg(feature = "module")]
be_wasm_guest_abi::guest!(Settling);
