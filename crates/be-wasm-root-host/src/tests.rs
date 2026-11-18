use std::{sync::Arc, time::Duration};

use be_lib_layers::{LayerId, LayerList, Surface};
use be_wasm_host::{Host, Stored, library_file_name, sandbox::Staged};
use be_wasm_root_testing::{
    CHILD_LAYER, CHILD_ORIGIN, HANGS_CHILD, HEARD_LAYER, PAINTED_SURFACE, PAINTER, PAINTER_ENTRY,
    PAINTER_LAYER, PAINTS, ROOT_LAYER, SPINS,
};

use super::*;

mod a_child_that_lets_its_surface_go_names_nothing_there_any_more;
mod a_frame_past_its_deadline_draws_without_the_child_it_waits_for;
mod a_frame_asks_its_host_for_what_the_surface_under_the_pointer_and_the_focus_ask;
mod a_frame_waits_for_its_child_and_shows_it;
mod a_root_built_on_the_library_shows_the_editor_it_embeds;
mod a_root_draws_its_own_surface_on_the_hosts_device;
mod a_root_names_what_its_child_presented_only_by_handle;
mod a_root_that_keeps_waking_itself_without_answering_stops_at_its_patience;
mod a_root_wants_frames_until_its_app_presents_and_wakes_its_host_for_them;
mod input_reaches_the_child_under_it_by_the_layers_drawn_last;
mod input_reaches_the_root_with_the_time_it_was_sampled;

const ROOT: &[u8] = include_bytes!(env!("ROOT_GUEST_WASM"));

/// The echo guest as a staged app, linked against the base library.
const ECHO_APP: &[u8] = include_bytes!(env!("GUEST_ECHO_APP_WASM"));

const BASE_LIBRARY: &[u8] = include_bytes!(env!("BASE_LIBRARY_WASM"));

const ECHO_ENTRY: &str = be_wasm_root_testing::CHILD_ENTRY;

/// The root built on be-sdk-root, which embeds the triangle's editor.
const EMBEDDING_ROOT: &[u8] = include_bytes!(env!("EMBEDDING_ROOT_WASM"));

/// The triangle as a staged app, linked against the base library.
const TRIANGLE_APP: &[u8] = include_bytes!(env!("TRIANGLE_WASM"));

/// The app a `PAINTS` frame spawns, as text.
const PAINTER_APP: &str = include_str!("tests/painter.wat");

/// Finds the staged modules: the echo app's and the triangle's entry points, each with the base
/// library staged as be-tool-buck stages them (the library named by its identity), and the
/// painter's, a standalone app that links no library.
fn staged(app_id: &str, module: &str) -> Result<Staged, String> {
    if app_id == PAINTER && module == PAINTER_ENTRY {
        let painter = wat::parse_str(PAINTER_APP).map_err(|error| error.to_string())?;
        return Ok(Staged::new(
            PAINTER_ENTRY.to_owned(),
            Arc::new(move |name| match name {
                PAINTER_ENTRY => Ok(Stored::Bytes(painter.clone())),
                _ => Err(format!("{name} is not staged")),
            }),
        ));
    }
    let (entry, app) = match (app_id, module) {
        (be_wasm_guest_echo::APP_ID, ECHO_ENTRY) => (ECHO_ENTRY, ECHO_APP),
        (be_sdk_root_testing::APP, be_sdk_root_testing::APP_ENTRY) => {
            (be_sdk_root_testing::APP_ENTRY, TRIANGLE_APP)
        }
        _ => return Err(format!("{app_id} stages no module named {module:?}")),
    };
    let library = library_file_name(&be_wasm_dylink::identity(BASE_LIBRARY));
    Ok(Staged::new(
        entry.to_owned(),
        Arc::new(move |name| match name {
            _ if name == entry => Ok(Stored::Bytes(app.to_vec())),
            _ if name == library => Ok(Stored::Bytes(BASE_LIBRARY.to_vec())),
            _ => Err(format!("{name} is not staged")),
        }),
    ))
}

/// The test root, started on this thread, its children drawing on a device of the host's.
fn root() -> Root {
    assert_eq!(
        be_wasm_root_testing::CHILD,
        be_wasm_guest_echo::APP_ID,
        "the root spawns the echo app"
    );
    start(ROOT)
}

/// The root `module`, started on this thread, it and its children drawing on a stand-in for the
/// host's device, which takes their work and draws nothing.
fn start(module: &[u8]) -> Root {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let app = Host::pure()
        .and_then(|host| {
            host.with_display(device.clone(), queue.clone())
                .load_bytes(module)
        })
        .expect("the root loads");
    Root::start(app, Box::new(staged), device, queue).expect("the root starts")
}

/// The ids of the layers a frame shows, bottom first.
fn shown(done: &FrameDone) -> Vec<LayerId> {
    LayerList::decode(&done.layers)
        .expect("the frame is a layer list")
        .windows
        .iter()
        .flat_map(|window| window.layers.iter().map(|layer| layer.id))
        .collect()
}

/// Far enough off that a child that answers at all answers before it.
const PATIENT: Duration = Duration::from_secs(60);
