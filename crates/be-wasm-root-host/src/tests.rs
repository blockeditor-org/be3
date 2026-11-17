use std::time::Duration;

use be_lib_layers::{LayerId, LayerList};
use be_wasm_host::{Host, Stored, library_file_name, sandbox::Staged};
use be_wasm_root_testing::{CHILD_LAYER, HANGS_CHILD, ROOT_LAYER, SPINS};

use super::*;

mod a_frame_past_its_deadline_draws_without_the_child_it_waits_for;
mod a_frame_waits_for_its_child_and_shows_it;
mod a_root_that_keeps_waking_itself_without_answering_stops_at_its_patience;

const ROOT: &[u8] = include_bytes!(env!("ROOT_GUEST_WASM"));

/// The echo guest as a staged app, linked against the base library.
const ECHO_APP: &[u8] = include_bytes!(env!("GUEST_ECHO_APP_WASM"));

const BASE_LIBRARY: &[u8] = include_bytes!(env!("BASE_LIBRARY_WASM"));

const ECHO_ENTRY: &str = be_wasm_root_testing::CHILD_ENTRY;

/// Finds the one staged module, the echo app's entry point, with the base library staged as
/// be-tool-buck stages them: the library named by its identity.
fn staged(app_id: &str, module: &str) -> Result<Staged, String> {
    if app_id != be_wasm_guest_echo::APP_ID || module != ECHO_ENTRY {
        return Err(format!("{app_id} stages no module named {module:?}"));
    }
    let library = library_file_name(&be_wasm_dylink::identity(BASE_LIBRARY));
    Ok(Staged::new(
        ECHO_ENTRY.to_owned(),
        Arc::new(move |name| match name {
            ECHO_ENTRY => Ok(Stored::Bytes(ECHO_APP.to_vec())),
            _ if name == library => Ok(Stored::Bytes(BASE_LIBRARY.to_vec())),
            _ => Err(format!("{name} is not staged")),
        }),
    ))
}

/// The test root, started on this thread.
fn root() -> Root {
    assert_eq!(
        be_wasm_root_testing::CHILD,
        be_wasm_guest_echo::APP_ID,
        "the root spawns the echo app"
    );
    let app = Host::pure()
        .and_then(|host| host.load_bytes(ROOT))
        .expect("the root loads");
    Root::start(app, Box::new(staged)).expect("the root starts")
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
