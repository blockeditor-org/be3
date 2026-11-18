use std::{sync::Arc, time::Duration};

use be_lib_layers::{LayerId, LayerList, Surface};
use be_wasm_host::{Host, Stored, library_file_name, sandbox::Staged};
use be_wasm_root_testing::{
    CHILD_LAYER, CHILD_ORIGIN, HANGS_CHILD, HEARD_LAYER, PAINTED_SURFACE, PAINTER, PAINTER_ENTRY,
    PAINTER_LAYER, PAINTS, ROOT_LAYER, SPINS,
};

use super::*;

mod a_child_that_lets_its_surface_go_names_nothing_there_any_more;
mod a_root_asks_to_keep_display_settings_its_host_changed;
mod a_root_lists_its_hosts_unsent_edits_and_asks_before_discarding_one;
mod a_dialog_shows_over_its_scrim_which_captures_a_press_beside_it_and_closes_it;
mod a_frame_asks_its_host_for_what_the_surface_under_the_pointer_and_the_focus_ask;
mod a_frame_past_its_deadline_draws_without_the_child_it_waits_for;
mod a_frame_waits_for_its_child_and_shows_it;
mod a_root_built_on_the_library_shows_the_editor_it_embeds;
mod a_root_draws_its_own_surface_on_the_hosts_device;
mod a_root_names_what_its_child_presented_only_by_handle;
mod a_root_paints_its_own_surface_with_beui_in_the_hosts_fonts;
mod a_root_shows_its_hosts_lock_screen_and_hands_over_only_what_was_typed;
mod a_root_shows_its_hosts_notices_as_toasts_hit_only_where_they_are;
mod a_root_shows_its_hosts_statistics_in_a_panel_watching_them_while_it_is_open;
mod a_root_shows_the_slow_saves_toast_while_its_hosts_changes_stay_unsaved;
mod a_root_that_keeps_waking_itself_without_answering_stops_at_its_patience;
mod a_root_wants_frames_until_its_app_presents_and_wakes_its_host_for_them;
mod a_roots_document_draws_again_when_the_frame_it_asked_for_comes;
mod escape_closes_a_dialog_that_has_the_focus;
mod input_reaches_the_child_under_it_by_the_layers_drawn_last;
mod input_reaches_the_root_with_the_time_it_was_sampled;
mod typing_through_the_input_method_reaches_a_dialogs_field_and_the_clipboard_is_the_hosts;

const ROOT: &[u8] = include_bytes!(env!("ROOT_GUEST_WASM"));

/// The echo guest as a staged app, linked against the base library.
const ECHO_APP: &[u8] = include_bytes!(env!("GUEST_ECHO_APP_WASM"));

const BASE_LIBRARY: &[u8] = include_bytes!(env!("BASE_LIBRARY_WASM"));

const BEUI_LIBRARY: &[u8] = include_bytes!(env!("BEUI_LIBRARY_WASM"));

const ECHO_ENTRY: &str = be_wasm_root_testing::CHILD_ENTRY;

/// The root built on be-sdk-root, which embeds the triangle's editor and draws its own surface
/// with beui, linked against the base and beui libraries.
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

/// The pure root `module`, started on this thread, it and its children drawing on a stand-in for
/// the host's device, which takes their work and draws nothing.
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

/// The embedding root, linked against the base and beui libraries and loaded as be-host loads
/// a root, as an app is, started on this thread with beui's text face and icons, it and its
/// children drawing on a stand-in for the host's device.
fn embedding() -> Root {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let base = be_wasm_dylink::identity(BASE_LIBRARY);
    let beui = be_wasm_dylink::identity(BEUI_LIBRARY);
    let app = Host::new(device.clone(), queue.clone())
        .and_then(|host| {
            host.load_bytes_with_libraries(EMBEDDING_ROOT, |identity| match identity {
                _ if identity == base => Ok(BASE_LIBRARY.to_vec()),
                _ if identity == beui => Ok(BEUI_LIBRARY.to_vec()),
                _ => Err(format!("no library {identity} is staged")),
            })
        })
        .expect("the root loads, linked against its libraries");
    Root::start(app, Box::new(staged), device, queue)
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

/// What beui painted of be-sdk-root-testing's strip of text (`TEXT_SURFACE`), asked as
/// `own_paintings` asks.
fn text_painting(root: &mut Root, time: u64) -> beui_paint_format::Painting {
    own_paintings(root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(be_sdk_root_testing::TEXT_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its strip of text")
}

/// Every glyph `primitives` paint, in groups too, by its rect.
fn glyphs(primitives: &[beui_paint_format::Primitive]) -> Vec<[f32; 4]> {
    primitives
        .iter()
        .flat_map(|primitive| match &primitive.content {
            beui_paint_format::Content::Glyph(glyph) => vec![glyph.rect],
            beui_paint_format::Content::Group(group) => glyphs(&group.primitives),
            _ => Vec::new(),
        })
        .collect()
}

/// Far enough off that a child that answers at all answers before it.
const PATIENT: Duration = Duration::from_secs(60);

/// The ids of the layers a frame shows, bottom first.
fn shown(done: &FrameDone) -> Vec<LayerId> {
    LayerList::decode(&done.layers)
        .expect("the frame is a layer list")
        .windows
        .iter()
        .flat_map(|window| window.layers.iter().map(|layer| layer.id))
        .collect()
}

/// Draws a frame of the embedding root for the frame clock's `time`.
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

/// Opens the embedding root's dialog with the key it intercepts, from the frame clock's `*time`,
/// and answers the first frame in which the dialog has the focus and its field the input method.
fn open_dialog(root: &mut Root, time: &mut u64) -> FrameDone {
    until(
        root,
        time,
        chord(
            be_sdk_root_testing::OPENS_DIALOG,
            be_lib_ui_types::Modifiers::NONE,
        ),
        "F2 opens the dialog, which takes the focus for its field",
        |done| layer_of(done, be_sdk_root_testing::DIALOG_LAYER).is_some() && done.ime.is_some(),
    )
}
