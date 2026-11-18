use std::time::{Duration, Instant};

use be_lib_layers::{LayerId, LayerList};
use be_wasm_host::Host;
use be_wasm_root_api::{FrameDone, FrameRequest};
use be_wasm_root_host::{Password, Root};

use crate::{MENU_ORIGIN, MENU_SURFACE};

mod a_root_asks_before_leaving_a_block_and_opens_its_hosts_contact_and_share_links;
mod a_root_asks_its_hosts_questions_and_shows_an_invitations_link_only_once;
mod a_root_asks_to_keep_display_settings_its_host_changed;
mod a_root_lists_its_hosts_members_and_agents_and_shows_an_agents_credentials_once;
mod a_root_lists_its_hosts_unsent_edits_and_asks_before_discarding_one;
mod a_root_opens_the_block_stacks_dialogs_from_its_menu;
mod a_root_shows_its_hosts_accounts_and_adds_one_handing_over_only_what_was_typed;
mod a_root_shows_its_hosts_lock_screen_and_statistics;
mod a_root_shows_its_hosts_organizations_and_sessions;
mod a_root_shows_its_hosts_recovery_phrase_and_unlocks_and_pairs_handing_over_only_what_was_typed;
mod a_root_shows_the_slow_saves_toast_while_its_hosts_changes_stay_unsaved;

/// be-root, linked against the base and beui libraries.
const BE_ROOT: &[u8] = include_bytes!(env!("BE_ROOT_WASM"));

const BASE_LIBRARY: &[u8] = include_bytes!(env!("BASE_LIBRARY_WASM"));

const BEUI_LIBRARY: &[u8] = include_bytes!(env!("BEUI_LIBRARY_WASM"));

/// Far enough off that a root that answers at all answers before it.
const PATIENT: Duration = Duration::from_secs(60);

/// What the root's menu holds, in its order, which `menu` picks by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    Unsent,
    Contacts,
    ShareLink,
    Invitations,
    Members,
    Agents,
    Sessions,
    SwitchOrganization,
    NewRecoveryPhrase,
    ManageAccounts,
    Lock,
    Performance,
}

/// be-root loaded as be-host loads a root, as an app is, started on this thread with beui's
/// text face and icons, drawing on a stand-in for the host's device. It embeds no app, so
/// nothing is staged for it.
fn embedding() -> Root {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let base = be_wasm_dylink::identity(BASE_LIBRARY);
    let beui = be_wasm_dylink::identity(BEUI_LIBRARY);
    let app = Host::new(device.clone(), queue.clone())
        .and_then(|host| {
            host.load_bytes_with_libraries(BE_ROOT, |identity| match identity {
                _ if identity == base => Ok(BASE_LIBRARY.to_vec()),
                _ if identity == beui => Ok(BEUI_LIBRARY.to_vec()),
                _ => Err(format!("no library {identity} is staged")),
            })
        })
        .expect("the root loads, linked against its libraries");
    Root::start(
        app,
        Box::new(|app_id: &str, module: &str| {
            Err(format!("{app_id} stages no module named {module:?}"))
        }),
        device,
        queue,
    )
    .expect("the root starts")
    .with_fonts(fonts())
}

/// The fonts the tests' host gives its root: beui's text face and its icons, as be-host hands
/// its bundled fonts (be-sdk-api's `Fonts`, encoded).
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

/// What the root painted of each of its own surfaces, by id, asked with a frame for `time`, the
/// time of the last.
fn own_paintings(root: &mut Root, time: u64) -> Vec<(u64, beui_paint_format::Painting)> {
    root.ask_paintings().expect("the root runs");
    root.frame(
        FrameRequest {
            time,
            wall_clock: 0,
        },
        Instant::now() + PATIENT,
    )
    .expect("the root draws");
    let painted = root
        .take_paintings()
        .expect("the root answered with the frame it stepped through");
    let painted =
        beui_paint_format::ScreenPaintings::decode(&painted).expect("the root's paintings decode");
    painted.screens
}

/// What the root paints `surface` with, a few frames from `*time` on.
fn painting_of(root: &mut Root, time: &mut u64, surface: u32) -> beui_paint_format::Painting {
    for _ in 0..3 {
        *time += 1;
        step(root, *time);
    }
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(surface))
        .map(|(_, painting)| painting)
        .expect("the root paints the surface")
}

/// The ids of the layers a frame shows, bottom first.
fn shown(done: &FrameDone) -> Vec<LayerId> {
    LayerList::decode(&done.layers)
        .expect("the frame is a layer list")
        .windows
        .iter()
        .flat_map(|window| window.layers.iter().filter(|layer| layer.shown))
        .map(|layer| layer.id)
        .collect()
}

/// Draws a frame of the root for the frame clock's `time`.
fn step(root: &mut Root, time: u64) -> FrameDone {
    root.frame(
        FrameRequest {
            time,
            wall_clock: 0,
        },
        Instant::now() + PATIENT,
    )
    .expect("the root draws")
}

/// The layer `id` of a frame's one window, if it shows.
fn layer_of(done: &FrameDone, id: LayerId) -> Option<be_lib_layers::Layer> {
    LayerList::decode(&done.layers)
        .expect("the frame is a layer list")
        .windows
        .into_iter()
        .flat_map(|window| window.layers)
        .find(|layer| layer.id == id && layer.shown)
}

/// The key `key` pressed and let go, with `modifiers` held around it.
fn chord(
    key: be_lib_ui_types::Key,
    modifiers: be_lib_ui_types::Modifiers,
) -> Vec<be_lib_ui_types::input::Timed> {
    use be_lib_ui_types::input::{Input, Keystroke};
    let stroke = |pressed| {
        Input::Key(Keystroke {
            key: Some(key),
            pressed,
            ..Keystroke::default()
        })
        .into()
    };
    let held = modifiers != be_lib_ui_types::Modifiers::NONE;
    let mut events = Vec::new();
    if held {
        events.push(Input::Modifiers(modifiers).into());
    }
    events.extend([stroke(true), stroke(false)]);
    if held {
        events.push(Input::Modifiers(be_lib_ui_types::Modifiers::NONE).into());
    }
    events
}

/// A press and release of the primary button at `at`, in the window's points.
fn click(at: be_lib_geometry::Pos2) -> Vec<be_lib_ui_types::input::Timed> {
    use be_lib_ui_types::input::{Input, PointerButton};
    let button = |pressed| {
        Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
        }
        .into()
    };
    vec![Input::PointerMoved(at).into(), button(true), button(false)]
}

/// Hands the root `events`, then draws frames from `*time` on until `done` holds of one, which
/// it answers: within a few frames, or the test fails saying `what`.
fn until(
    root: &mut Root,
    time: &mut u64,
    events: Vec<be_lib_ui_types::input::Timed>,
    what: &str,
    done: impl Fn(&FrameDone) -> bool,
) -> FrameDone {
    if !events.is_empty() {
        root.input(None, be_lib_ui_types::input::encode(&events))
            .expect("the root takes input");
    }
    for _ in 0..12 {
        *time += 1;
        let drawn = step(root, *time);
        if done(&drawn) {
            return drawn;
        }
    }
    panic!("within a few frames, {what}");
}

/// Hands the root `events` and draws a few frames.
fn hand(root: &mut Root, time: &mut u64, events: Vec<be_lib_ui_types::input::Timed>) {
    if !events.is_empty() {
        root.input(None, be_lib_ui_types::input::encode(&events))
            .expect("the root takes input");
    }
    for _ in 0..4 {
        *time += 1;
        step(root, *time);
    }
}

/// Where the root's menu button is, in the window's points: at its top right.
fn menu_button() -> be_lib_geometry::Pos2 {
    be_lib_geometry::pos2(MENU_ORIGIN.0 + 256.0, MENU_ORIGIN.1 + 24.0)
}

/// Opens the root's menu with a press of its button, then picks `entry` from it with the
/// keyboard, as a person may, answering what it asked of the block stack.
fn menu(root: &mut Root, time: &mut u64, entry: Entry) -> Vec<be_wasm_root_api::CoreCommand> {
    hand(root, time, click(menu_button()));
    for _ in 0..=entry as usize {
        hand(
            root,
            time,
            chord(
                be_lib_ui_types::Key::ArrowDown,
                be_lib_ui_types::Modifiers::NONE,
            ),
        );
    }
    hand(
        root,
        time,
        chord(
            be_lib_ui_types::Key::Enter,
            be_lib_ui_types::Modifiers::NONE,
        ),
    );
    root.take_core_commands()
}

/// What the root paints its menu with, a few frames from `*time` on.
fn menu_painting(root: &mut Root, time: &mut u64) -> beui_paint_format::Painting {
    painting_of(root, time, MENU_SURFACE)
}
