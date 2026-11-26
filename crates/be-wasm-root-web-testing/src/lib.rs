//! The web's half of the root entry and of the sandbox in Chromium
//! (plans/host_as_platform_adapter.md, step 4d): the page be-wasm-root-host's `web` runs a root
//! on. `run`, which web/index.html calls, runs 4a's root (be-wasm-root-testing) inline on the
//! page from `requestAnimationFrame`, as its native tests run it, with its child, the echo
//! guest as a staged app, in a web worker; and serves a guest on the page the sandbox
//! (be-wasm-sandbox-host's `Server`), which compiles the pure echo guest from bytes and talks to
//! an instance of it in a plain worker. Nothing here waits: every wait is a promise.

#![cfg(target_arch = "wasm32")]

use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use be_lib_layers::{LayerId, LayerList, Surface};
use be_wasm_guest_host::Wake;
use be_wasm_ops::{Connection, Incoming, Side};
use be_wasm_root_api::{FrameDone, FrameRequest};
use be_wasm_root_host::{Root, Staged};
use be_wasm_root_testing::{
    CHILD, CHILD_ENTRY, CHILD_LAYER, HANGS_CHILD, ROOT_LAYER, SPINS, WAKES_ONCE,
};
use be_wasm_sandbox_api::{
    embed, guest,
    sandbox::{Config, FromBytes, Source},
};
use be_wasm_sandbox_host::Server;
use js_sys::{Function, Promise};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;

/// The root guest, staged beside the bundle by web/BUCK's page test.
const ROOT: &str = "../root.wasm";

/// The echo guest as a pure guest's module, which the sandbox compiles from its bytes.
const ECHO: &str = "../echo-pure.wasm";

/// The guest be-wasm-host's tests run natively (its src/tests/pure_wasi.wat), which imports every
/// WASI call a pure guest is answered, and one that would read a file.
const PURE_WASI: &str = "../pure-wasi.wasm";
const READS_A_FILE: &str = "../reads-a-file.wasm";

/// The root built on be-sdk-root (be-sdk-root-testing), linked against the base and beui
/// libraries, staged in the bundle beside app.js and the libraries it links.
const EMBEDDING_ROOT: &str = "./root_testing.wasm";

/// Far enough off that a child that answers at all answers before it.
const PATIENT: Duration = Duration::from_secs(30);

/// Runs every check, answering what passed or why one failed.
#[wasm_bindgen]
pub async fn run() -> Result<String, JsValue> {
    be_wasm_threads::initialize_page();
    let mut passed = Vec::new();
    passed.push(a_frame_waits_for_its_child_and_shows_it().await?);
    passed.push(a_linked_root_draws_its_own_surface_on_the_page().await?);
    passed.push(an_app_a_linked_root_spawns_draws_into_a_canvas_its_worker_is_handed().await?);
    passed.push(a_frame_past_its_deadline_draws_without_the_child_it_waits_for().await?);
    passed.push(a_root_with_something_to_step_between_frames_wakes_its_page().await?);
    passed.push(a_root_that_keeps_waking_itself_stops_at_its_patience().await?);
    passed.push(a_root_that_keeps_waking_itself_lets_the_page_run_its_tasks().await?);
    passed.push(a_guest_compiles_a_module_and_talks_to_an_instance_of_it().await?);
    passed.push(a_module_that_imports_more_than_a_pure_guest_may_does_not_compile().await?);
    passed.push(an_instance_of_a_module_from_bytes_is_held_to_its_memory_limit().await?);
    passed.push(a_module_from_bytes_is_answered_a_pure_guests_wasi_as_natively().await?);
    passed.push(a_module_from_bytes_that_reads_a_file_does_not_compile().await?);
    Ok(passed.join("; "))
}

/// Finds the one staged module, the echo app's entry point, staged in the bundle with the base
/// library it links.
fn staged(app: &str, module: &str) -> Result<Staged, String> {
    if app != CHILD || module != CHILD_ENTRY {
        return Err(format!("{app} stages no module named {module:?}"));
    }
    Ok(Staged::new(CHILD_ENTRY.to_owned()))
}

/// be-sdk-root-testing's app, the triangle, staged in the bundle with the base library it links.
const TRIANGLE: &str = "be3.triangle";
const TRIANGLE_ENTRY: &str = "triangle.wasm";

/// Finds the triangle, the one app the linked root spawns.
fn triangle(app: &str, module: &str) -> Result<Staged, String> {
    if app != TRIANGLE || module != TRIANGLE_ENTRY {
        return Err(format!("{app} stages no module named {module:?}"));
    }
    Ok(Staged::new(TRIANGLE_ENTRY.to_owned()))
}

async fn root() -> Result<Root, String> {
    // A page delivers Ctrl+V as a paste from the browser's paste event, as every page that runs
    // a root does.
    Ok(Root::load(ROOT, Box::new(staged))
        .await?
        .with_paste_shortcut(true))
}

/// The page's window as the root is told it: the test root draws a fixed size anyway.
fn window() -> be_wasm_root_api::WindowSize {
    be_wasm_root_api::WindowSize {
        width: 400.0,
        height: 240.0,
        pixels_per_point: 1.0,
    }
}

/// One frame of `root`, started from the next `requestAnimationFrame` with its deadline `budget`
/// after it, as the page would draw one.
async fn frame(
    root: &Rc<Root>,
    time: u64,
    budget: Duration,
) -> Result<Result<FrameDone, String>, JsValue> {
    let (drawn, answer) = promised::<Result<FrameDone, String>>();
    let root = Rc::clone(root);
    let draw = Closure::once_into_js(move || {
        let deadline = Instant::now() + budget;
        let started = root.frame(
            FrameRequest {
                time,
                wall_clock: 0,
                utc_offset: 0,
                preferences: Default::default(),
                window: window(),
                describe: false,
                phone: false,
            },
            deadline,
            move |done| answer(done),
        );
        assert!(started, "no earlier frame is still being drawn");
    });
    web_sys::window()
        .ok_or("no window")?
        .request_animation_frame(draw.unchecked_ref())?;
    Ok(drawn.await)
}

/// The ids of the layers a frame shows, bottom first.
fn shown(done: &FrameDone) -> Result<Vec<LayerId>, String> {
    Ok(LayerList::decode(&done.layers)
        .map_err(|error| format!("the frame is no layer list: {error}"))?
        .windows
        .iter()
        .flat_map(|window| window.layers.iter().map(|layer| layer.id))
        .collect())
}

fn check(holds: bool, why: impl FnOnce() -> String) -> Result<(), JsValue> {
    if holds { Ok(()) } else { Err(why().into()) }
}

/// The fonts the page gives the linked root: beui's text face and its icons, as be-host hands its
/// bundled fonts (be-sdk-api's `Fonts`, encoded).
fn fonts() -> Vec<u8> {
    let face = |role, data: &[u8]| be_sdk_api::FontFace {
        role,
        index: 0,
        data: data.to_vec(),
    };
    be_wasm_ops::encode(&be_sdk_api::Fonts {
        replace: true,
        faces: vec![
            face(be_sdk_api::FontRole::Text, beui_fonts::INTER_REGULAR),
            face(be_sdk_api::FontRole::Icons, beui_fonts::ICONS),
        ],
    })
}

/// The colour at the middle of what `canvas` shows, read back through a canvas of its own.
fn middle(canvas: &web_sys::OffscreenCanvas) -> Result<[u8; 4], JsValue> {
    let bitmap = canvas.transfer_to_image_bitmap()?;
    let (width, height) = (bitmap.width(), bitmap.height());
    let reader = web_sys::OffscreenCanvas::new(width, height)?;
    let context: web_sys::OffscreenCanvasRenderingContext2d = reader
        .get_context("2d")?
        .ok_or("no 2d context")?
        .unchecked_into();
    context.draw_image_with_image_bitmap(&bitmap, 0.0, 0.0)?;
    let pixel = context
        .get_image_data(f64::from(width / 2), f64::from(height / 2), 1.0, 1.0)?
        .data();
    Ok([pixel[0], pixel[1], pixel[2], pixel[3]])
}

async fn a_linked_root_draws_its_own_surface_on_the_page() -> Result<String, JsValue> {
    // be-sdk-root-testing's, which the page does not link: the module is its guest's.
    const EMBEDDING_LAYER: LayerId = LayerId(0);
    const ROOT_SURFACE: u32 = 0;
    const ROOT_COLOR: [f64; 4] = [0.3, 0.1, 0.4, 1.0];
    const SIZE: (u32, u32) = (400, 240);
    let root = Rc::new(
        Root::load(EMBEDDING_ROOT, Box::new(triangle))
            .await?
            .with_fonts(fonts())
            .with_patience(PATIENT),
    );
    // The gpu shim serves one linked root a page: a second waits for the first to go.
    let second = Root::load(EMBEDDING_ROOT, Box::new(triangle)).await;
    check(second.is_err(), || {
        "a second linked root started on a page whose first still runs".to_owned()
    })?;
    let canvas = web_sys::OffscreenCanvas::new(SIZE.0, SIZE.1)?;
    root.show_surface(1, &canvas, ROOT_SURFACE);
    let done = frame(&root, 1, Duration::from_secs(2)).await??;
    let list = LayerList::decode(&done.layers)
        .map_err(|error| format!("the frame is no layer list: {error}"))?;
    let own = list
        .windows
        .iter()
        .flat_map(|window| window.layers.iter())
        .find(|layer| layer.id == EMBEDDING_LAYER)
        .ok_or("the frame shows no layer of the root's own")?;
    check(own.surface == Surface::Root(ROOT_SURFACE), || {
        format!("the root's layer shows {:?}, not its own surface", own.surface)
    })?;
    root.paint()?;
    let shown = middle(&canvas)?;
    // ROOT_COLOR, sRGB-encoded as the surface holds it, to the byte, give or take one.
    let wanted = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    let close = shown
        .iter()
        .zip(wanted)
        .all(|(shown, wanted)| shown.abs_diff(wanted) <= 1);
    check(close, || {
        format!("the canvas showing the root's surface shows {shown:?}, not {wanted:?}")
    })?;
    drop(root);
    let again = Root::load(EMBEDDING_ROOT, Box::new(triangle)).await;
    check(again.is_ok(), || {
        format!("a linked root could not start once the first was dropped: {:?}", again.err())
    })?;
    Ok("a linked root drew its own surface on the page's device, which a canvas showed; a \
        second waited for it to go"
        .to_owned())
}

async fn an_app_a_linked_root_spawns_draws_into_a_canvas_its_worker_is_handed()
-> Result<String, JsValue> {
    let root = Rc::new(
        Root::load(EMBEDDING_ROOT, Box::new(triangle))
            .await?
            .with_fonts(fonts())
            .with_patience(PATIENT),
    );
    // The first frame waits for the triangle to start and show its screen.
    let done = frame(&root, 1, PATIENT).await??;
    let list = LayerList::decode(&done.layers)
        .map_err(|error| format!("the frame is no layer list: {error}"))?;
    let screen = list
        .windows
        .iter()
        .flat_map(|window| window.layers.iter())
        .find(|layer| matches!(layer.surface, Surface::Screen { .. }))
        .ok_or("the frame shows no screen of the triangle's")?;
    let Surface::Screen { instance, surface } = screen.surface else {
        return Err("the triangle's layer shows no screen".into());
    };
    // A canvas on the page whose drawing its worker takes over, as be-host's web host places an
    // app's screens.
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("no document")?;
    let element: web_sys::HtmlCanvasElement = document.create_element("canvas")?.unchecked_into();
    element.set_width(screen.size.0);
    element.set_height(screen.size.1);
    document.body().ok_or("no body")?.append_child(&element)?;
    // Whatever the test finds, the canvas leaves the page with it.
    let shown = async {
        let canvas = element.transfer_control_to_offscreen()?;
        root.show_app_surface(2, &canvas, instance, surface)?;
        // The worker draws into it what the triangle presents, from its next step on, in its
        // own time: the page sees it once the worker's frame reaches the screen.
        let mut shown = [0; 4];
        for time in 2..40 {
            frame(&root, time, Duration::from_millis(500)).await??;
            shown = middle_of(&element)?;
            if shown[3] != 0 {
                break;
            }
        }
        Ok::<_, JsValue>(shown)
    }
    .await;
    element.remove();
    let shown = shown?;
    check(shown[3] != 0, || {
        format!("the canvas the triangle's worker was handed shows nothing at its middle: {shown:?}")
    })?;
    Ok("an app a linked root spawned drew its screen into a canvas its worker was handed".to_owned())
}

/// The colour at the middle of what the canvas element shows, whose drawing a worker took over.
fn middle_of(element: &web_sys::HtmlCanvasElement) -> Result<[u8; 4], JsValue> {
    let (width, height) = (element.width(), element.height());
    let reader = web_sys::OffscreenCanvas::new(width, height)?;
    let context: web_sys::OffscreenCanvasRenderingContext2d = reader
        .get_context("2d")?
        .ok_or("no 2d context")?
        .unchecked_into();
    context.draw_image_with_html_canvas_element(element, 0.0, 0.0)?;
    let pixel = context
        .get_image_data(f64::from(width / 2), f64::from(height / 2), 1.0, 1.0)?
        .data();
    Ok([pixel[0], pixel[1], pixel[2], pixel[3]])
}

async fn a_frame_waits_for_its_child_and_shows_it() -> Result<String, JsValue> {
    let root = Rc::new(root().await?);
    for time in [1, 2] {
        let done = frame(&root, time, PATIENT).await??;
        let shown = shown(&done)?;
        check(shown == vec![ROOT_LAYER, CHILD_LAYER], || {
            format!("frame {time} showed {shown:?}, not the child over the root")
        })?;
    }
    Ok("two frames waited for their child, in a web worker, and showed it".to_owned())
}

async fn a_frame_past_its_deadline_draws_without_the_child_it_waits_for() -> Result<String, JsValue>
{
    let root = Rc::new(root().await?.with_patience(PATIENT));
    let started = Instant::now();
    // The child hangs, so it never answers: the frame ends at its deadline, which has passed.
    let done = frame(&root, HANGS_CHILD, Duration::ZERO).await??;
    check(started.elapsed() < PATIENT / 6, || {
        format!(
            "the frame took {}ms, past its deadline",
            started.elapsed().as_millis()
        )
    })?;
    let shown = shown(&done)?;
    check(shown == vec![ROOT_LAYER], || {
        format!("the frame past its deadline showed {shown:?}, not the root alone")
    })?;
    Ok("a frame past its deadline drew the root alone".to_owned())
}

async fn a_root_with_something_to_step_between_frames_wakes_its_page() -> Result<String, JsValue>
{
    let root = Rc::new(root().await?.with_patience(PATIENT));
    // The frame ends at its deadline, which has passed: the child it spawned answers after it,
    // between frames, which the page hears of.
    let woken = next_wake(&root);
    frame(&root, 1, Duration::ZERO).await??;
    woken.await;
    // The frame the page draws for it steps the root with what arrived.
    let done = frame(&root, 2, PATIENT).await??;
    let shown = shown(&done)?;
    check(shown == vec![ROOT_LAYER, CHILD_LAYER], || {
        format!("the frame drawn for the wake showed {shown:?}, not the child over the root")
    })?;
    // A root that wakes itself in the step that answers its frame has something to step for
    // once the frame ends, though the wake came while it was stepped.
    let woken = next_wake(&root);
    frame(&root, WAKES_ONCE, PATIENT).await??;
    woken.await;
    Ok("a root with something to step between frames, or once its frame ended, woke its page, \
        which drew it a frame"
        .to_owned())
}

/// Answers the first time the root's `on_wake` is called from now on.
fn next_wake(root: &Root) -> impl Future<Output = ()> {
    let (woken, answer) = promised::<()>();
    let answer = RefCell::new(Some(answer));
    root.on_wake(move || {
        if let Some(answer) = answer.borrow_mut().take() {
            answer(());
        }
    });
    woken
}

async fn a_root_that_keeps_waking_itself_stops_at_its_patience() -> Result<String, JsValue> {
    let root = Rc::new(root().await?.with_patience(Duration::from_millis(100)));
    let drawn = frame(&root, SPINS, Duration::ZERO).await?;
    check(drawn.is_err(), || {
        "the frame of a root that never answers was drawn".to_owned()
    })?;
    let again = frame(&root, 1, PATIENT).await?;
    check(again.is_err(), || {
        "a root that stopped drew another frame".to_owned()
    })?;
    Ok("a root that kept waking itself stopped at its patience".to_owned())
}

async fn a_root_that_keeps_waking_itself_lets_the_page_run_its_tasks() -> Result<String, JsValue> {
    let patience = Duration::from_secs(2);
    let root = Rc::new(root().await?.with_patience(patience));
    let (drawn, answer) = promised::<(Instant, Result<FrameDone, String>)>();
    let ran: Rc<RefCell<Option<Instant>>> = Rc::default();
    let started: Rc<RefCell<Option<Instant>>> = Rc::default();
    let draw = {
        let (ran, started, root) = (Rc::clone(&ran), Rc::clone(&started), Rc::clone(&root));
        Closure::once_into_js(move || {
            // A task the page queued before the root starts its frame, which runs only if the
            // root lets go of the page's thread.
            let task = Closure::once_into_js(move || *ran.borrow_mut() = Some(Instant::now()));
            let global = js_sys::global();
            if let Ok(set) = js_sys::Reflect::get(&global, &"setTimeout".into())
                && let Ok(set) = set.dyn_into::<Function>()
            {
                let _ = set.call2(&global, &task, &0.into());
            }
            *started.borrow_mut() = Some(Instant::now());
            let begun = root.frame(
                FrameRequest {
                    time: SPINS,
                    wall_clock: 0,
                    utc_offset: 0,
                    preferences: Default::default(),
                    window: window(),
                    describe: false,
                    phone: false,
                },
                Instant::now(),
                move |done| answer((Instant::now(), done)),
            );
            assert!(begun, "no earlier frame is still being drawn");
        })
    };
    web_sys::window()
        .ok_or("no window")?
        .request_animation_frame(draw.unchecked_ref())?;
    let (ended, done) = drawn.await;
    check(done.is_err(), || {
        "the frame of a root that never answers was drawn".to_owned()
    })?;
    let started = started.borrow().ok_or("the frame never started")?;
    let ran = (*ran.borrow()).ok_or("the page's task never ran while the root spun")?;
    check(ran < ended && ran - started < patience / 4, || {
        format!(
            "the page's task ran {}ms into the root's frame, which ended at {}ms",
            (ran - started).as_millis(),
            (ended - started).as_millis()
        )
    })?;
    Ok("a root that kept waking itself let the page run its tasks".to_owned())
}

async fn an_instance_of_a_module_from_bytes_is_held_to_its_memory_limit() -> Result<String, JsValue>
{
    let bytes = fetch(ECHO).await?;
    let mut pump = Pump::new();
    let (small, grown) = pump
        .run(async move {
            let module = guest::sandbox::compile(bytes)
                .await
                .map_err(|error| error.to_string())??;
            let echo = |memory_limit| {
                guest::sandbox::Instance::new(
                    Source::Module(FromBytes {
                        module: module,
                        seed: [0; 32],
                    }),
                    Config {
                        name: "echo".to_owned(),
                        memory_limit,
                    },
                )
            };
            // One page: less than the echo guest's memory starts at.
            let small = echo(1 << 16)
                .next()
                .await
                .map_err(|error| error.to_string())?;
            // Enough to start, but not for the gigabyte it is asked to grow by.
            let growing = echo(16 << 20);
            growing
                .send(vec![b"grow".to_vec()])
                .await
                .map_err(|error| error.to_string())?;
            let grown = loop {
                match growing.next().await.map_err(|error| error.to_string())? {
                    Ok(frames) if frames.iter().any(|frame| frame.len() == 1) => {
                        break Ok(frames);
                    }
                    Ok(_) => {}
                    Err(why) => break Err(why),
                }
            };
            Ok::<_, String>((small, grown))
        })
        .await?;
    match small {
        Err(why) if why.contains("past its limit") => {}
        other => return Err(format!("a one-page instance answered {other:?}").into()),
    }
    check(grown.is_err(), || {
        "an instance grew past its memory limit".to_owned()
    })?;
    Ok("an instance of a module from bytes was held to its memory limit".to_owned())
}

async fn a_guest_compiles_a_module_and_talks_to_an_instance_of_it() -> Result<String, JsValue> {
    let bytes = fetch(ECHO).await?;
    let mut pump = Pump::new();
    let received = pump
        .run(async move {
            let module = guest::sandbox::compile(bytes)
                .await
                .map_err(|error| error.to_string())??;
            let instance = guest::sandbox::Instance::new(
                Source::Module(FromBytes {
                    module: module,
                    seed: [0; 32],
                }),
                Config {
                    name: "echo".to_owned(),
                    memory_limit: 16 << 20,
                },
            );
            instance
                .send(vec![b"hello".to_vec()])
                .await
                .map_err(|error| error.to_string())?;
            let mut received = Vec::new();
            while !received.contains(&b"hello".to_vec()) {
                received.extend(instance.next().await.map_err(|error| error.to_string())??);
            }
            Ok::<_, String>(received)
        })
        .await?;
    check(received.contains(&b"hello".to_vec()), || {
        format!("the echo guest answered {received:?}")
    })?;
    Ok("a module compiled from bytes echoed from a worker of its own".to_owned())
}

async fn a_module_that_imports_more_than_a_pure_guest_may_does_not_compile()
-> Result<String, JsValue> {
    // The echo guest as a staged app imports the whole of WASI and its libraries.
    let bytes = fetch(CHILD_ENTRY).await?;
    let mut pump = Pump::new();
    let compiled = pump
        .run(async move {
            guest::sandbox::compile(bytes)
                .await
                .map_err(|error| error.to_string())
        })
        .await?;
    match compiled {
        Err(why) if why.contains("a pure guest may not") => {
            Ok("a module that imports more than a pure guest may was refused".to_owned())
        }
        Err(why) => Err(format!("the module was refused for another reason: {why}").into()),
        Ok(_) => Err("a module that imports more than a pure guest may compiled".into()),
    }
}

async fn a_module_from_bytes_is_answered_a_pure_guests_wasi_as_natively() -> Result<String, JsValue>
{
    use be_wasm_pure_wasi::vectors;
    let bytes = fetch(PURE_WASI).await?;
    // Across a block's end and the next's start, as be-wasm-host's native test draws them.
    let draws: [u8; 5] = [10, 54, 60, 8, 60];
    let wanted = 4 + draws.iter().map(|length| *length as usize).sum::<usize>();
    let mut pump = Pump::new();
    let received = pump
        .run(async move {
            let module = guest::sandbox::compile(bytes)
                .await
                .map_err(|error| error.to_string())??;
            let instance = guest::sandbox::Instance::new(
                Source::Module(FromBytes {
                    module: module,
                    seed: [0; 32],
                }),
                Config {
                    name: "pure wasi".to_owned(),
                    memory_limit: 16 << 20,
                },
            );
            instance
                .send(draws.iter().map(|length| vec![0, *length]).collect())
                .await
                .map_err(|error| error.to_string())?;
            let mut received: Vec<Vec<u8>> = Vec::new();
            while received.iter().map(Vec::len).sum::<usize>() < wanted {
                received.extend(instance.next().await.map_err(|error| error.to_string())??);
            }
            Ok::<_, String>(received)
        })
        .await?;
    check(
        received.first().map(Vec::as_slice) == Some(&[0, 52, 0, 24]),
        || format!("the guest started with {:?}", received.first()),
    )?;
    let drawn = received[1..].concat();
    check(
        vectors::hex(&drawn[..64]) == vectors::ZERO_BLOCK_0
            && vectors::hex(&drawn[64..128]) == vectors::ZERO_BLOCK_1,
        || format!("the guest drew {}", vectors::hex(&drawn)),
    )?;
    Ok(
        "a module from bytes was answered a pure guest's WASI and drew ChaCha20's stream"
            .to_owned(),
    )
}

async fn a_module_from_bytes_that_reads_a_file_does_not_compile() -> Result<String, JsValue> {
    let bytes = fetch(READS_A_FILE).await?;
    let mut pump = Pump::new();
    let compiled = pump
        .run(async move {
            guest::sandbox::compile(bytes)
                .await
                .map_err(|error| error.to_string())
        })
        .await?;
    match compiled {
        Err(why) if why.contains("fd_read") => {
            Ok("a module from bytes that reads a file was refused".to_owned())
        }
        Err(why) => Err(format!("the module was refused for another reason: {why}").into()),
        Ok(_) => Err("a module from bytes that reads a file compiled".into()),
    }
}

/// A guest run in process against a `Server`, as a host serves one, which waits for the
/// sandbox's wakes on a promise.
struct Pump {
    executor: be_wasm_ops::guest::Executor,
    host: Connection,
    server: Server,
    to_guest: Vec<Vec<u8>>,
}

thread_local! {
    /// Whether the sandbox woke the pump, and what resolves its wait for it.
    static RUNG: RefCell<(bool, Option<Function>)> = const { RefCell::new((false, None)) };
}

fn ring() {
    let waiting = RUNG.with(|rung| {
        let mut rung = rung.borrow_mut();
        rung.0 = true;
        rung.1.take()
    });
    if let Some(resolve) = waiting {
        let _ = resolve.call0(&JsValue::UNDEFINED);
    }
}

/// Answers once `ring` was called since the last answer.
async fn rung() {
    let ready = RUNG.with(|rung| std::mem::take(&mut rung.borrow_mut().0));
    if ready {
        return;
    }
    let promise = Promise::new(&mut |resolve, _| {
        RUNG.with(|rung| rung.borrow_mut().1 = Some(resolve));
    });
    let _ = JsFuture::from(promise).await;
    RUNG.with(|rung| rung.borrow_mut().0 = false);
}

impl Pump {
    fn new() -> Self {
        let wake: Wake = Arc::new(ring);
        Self {
            executor: guest::executor(()),
            host: Connection::new(Side::Host),
            server: Server::new(wake, Box::new(staged)),
            to_guest: Vec::new(),
        }
    }

    /// Runs `task` in the guest, passing frames both ways, until it finishes.
    async fn run<T: 'static>(
        &mut self,
        task: impl Future<Output = Result<T, String>> + 'static,
    ) -> Result<T, String> {
        let done = Rc::new(RefCell::new(None));
        let finished = Rc::clone(&done);
        self.executor.enter(|| {
            be_wasm_ops::guest::spawn(async move {
                *finished.borrow_mut() = Some(task.await);
            });
        });
        loop {
            let mut from_guest = Vec::new();
            self.executor
                .step(std::mem::take(&mut self.to_guest), &mut from_guest)?;
            for frame in &from_guest {
                self.host
                    .receive(frame)
                    .map_err(|error| error.to_string())?;
            }
            while let Some(event) = self.host.next_event() {
                match event {
                    Incoming::Started(started) => {
                        if let Some(call) = embed::sandbox::Call::accept(&mut self.host, started) {
                            self.server.call(call, &mut self.host);
                        }
                    }
                    Incoming::Canceled { op } => {
                        self.server.canceled(op, &mut self.host);
                    }
                    _ => {}
                }
            }
            self.server.poll(&mut self.host);
            self.to_guest = self.host.take_outbound();
            if let Some(value) = done.borrow_mut().take() {
                return value;
            }
            if self.to_guest.is_empty() && from_guest.is_empty() {
                rung().await;
            }
        }
    }
}

/// A promise of a `T` and what answers it, once.
fn promised<T: 'static>() -> (impl Future<Output = T>, impl FnOnce(T)) {
    let slot = Rc::new(RefCell::new(None));
    let mut resolved = None;
    let promise = Promise::new(&mut |resolve, _| resolved = Some(resolve));
    let answer = {
        let slot = Rc::clone(&slot);
        move |value| {
            *slot.borrow_mut() = Some(value);
            if let Some(resolve) = resolved {
                let _ = resolve.call0(&JsValue::UNDEFINED);
            }
        }
    };
    let answered = async move {
        let _ = JsFuture::from(promise).await;
        slot.borrow_mut()
            .take()
            .expect("a promise resolves only once answered")
    };
    (answered, answer)
}

async fn fetch(url: &str) -> Result<Vec<u8>, JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await?
        .unchecked_into();
    if !response.ok() {
        return Err(format!("{url} could not be fetched: {}", response.status()).into());
    }
    let bytes = JsFuture::from(response.array_buffer()?).await?;
    Ok(js_sys::Uint8Array::new(&bytes).to_vec())
}
