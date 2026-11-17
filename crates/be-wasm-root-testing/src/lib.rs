//! The root guest be-wasm-root-host's tests run (plans/host_as_platform_adapter.md, The root
//! guest): it settles one child as the compositor settles its apps. On its first frame it spawns
//! the echo guest as a staged app (`CHILD`); each frame it asks the child to echo the frame's
//! time, and races that whole exchange against the frame's deadline. Its layer list shows its own
//! surface, and over it the child's screen only when the child echoed in time.

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::{Pin, pin},
    rc::Rc,
    task::Poll,
};

use be_lib_geometry::{Rect, pos2};
use be_lib_layers::{Affine, Layer, LayerId, LayerInput, LayerList, Surface, WindowLayers};
use be_wasm_guest_abi::{Flow, Guest, Waker};
use be_wasm_ops::guest::{Executor, LocalFuture};
use be_wasm_root_api::{
    FrameDone, FrameRequest,
    guest::{exports::frames, host},
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

/// The root's own surface, which every frame shows.
pub const ROOT_LAYER: LayerId = LayerId(0);

/// The child's screen, which a frame shows when the child echoed in time.
pub const CHILD_LAYER: LayerId = LayerId(1);

/// The window's size, in points.
const SIZE: (u32, u32) = (200, 100);

#[derive(Clone, Default)]
struct Frames {
    child: Rc<RefCell<Option<Rc<Instance>>>>,
    spinning: Rc<Cell<bool>>,
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
        Box::pin(async move {
            let deadline = pin!(host::deadline());
            let exchange = pin!(frames.exchange(request.time));
            let echoed = matches!(first(exchange, deadline).await, First::Left(true));
            FrameDone {
                layers: layers(echoed).encode(),
            }
        })
    }
}

impl Frames {
    /// Asks the child to echo `time`, spawning it first if need be, and answers whether it did.
    async fn exchange(&self, time: u64) -> bool {
        let child = self.child();
        if time == HANGS_CHILD {
            let _ = child.send(vec![HANG.to_vec()]).await;
        }
        let asked = time.to_le_bytes().to_vec();
        if child.send(vec![asked.clone()]).await.is_err() {
            return false;
        }
        loop {
            match child.next().await {
                Ok(Ok(frames)) if frames.contains(&asked) => return true,
                Ok(Ok(_)) => {}
                // It stopped, or the host stopped answering for it.
                _ => return false,
            }
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

fn layers(echoed: bool) -> LayerList {
    let layer = |id, surface, size: (u32, u32)| Layer {
        id,
        surface,
        size,
        transform: Affine::IDENTITY,
        clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    };
    let mut layers = vec![layer(ROOT_LAYER, Surface::Root(0), SIZE)];
    if echoed {
        layers.push(layer(
            CHILD_LAYER,
            Surface::Screen {
                instance: 0,
                surface: 0,
            },
            (SIZE.0 / 2, SIZE.1),
        ));
    }
    LayerList {
        windows: vec![WindowLayers {
            window: None,
            layers,
        }],
    }
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

be_wasm_guest_abi::guest!(Settling);
