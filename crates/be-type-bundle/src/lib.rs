use be_block_type_host::Source;
use be_sdk_type_guest::Registration;
use be_wasm_guest_abi::Guest;
use uuid::Uuid;

pub fn sources() -> Vec<Source> {
    vec![
        in_process::<be_type_audio::BlockTypes>("be3.audio", be_type_audio::registrations()),
        in_process::<be_type_example_beui_gallery::BlockTypes>(
            "be3.beui_gallery",
            be_type_example_beui_gallery::registrations(),
        ),
        in_process::<be_type_browser_tab::BlockTypes>(
            "be3.browser-tab",
            be_type_browser_tab::registrations(),
        ),
        in_process::<be_type_checklist::BlockTypes>(
            "be3.checklist",
            be_type_checklist::registrations(),
        ),
        in_process::<be_type_example_counter::BlockTypes>(
            "be3.counter",
            be_type_example_counter::registrations(),
        ),
        in_process::<be_type_database::BlockTypes>(
            "be3.database",
            be_type_database::registrations(),
        ),
        in_process::<be_type_deterministic_game::BlockTypes>(
            "be3.deterministic-game",
            be_type_deterministic_game::registrations(),
        ),
        in_process::<be_type_image::BlockTypes>("be3.image", be_type_image::registrations()),
        in_process::<be_type_infinite_canvas::BlockTypes>(
            "be3.infinite-canvas",
            be_type_infinite_canvas::registrations(),
        ),
        in_process::<be_type_logic_grid::BlockTypes>(
            "be3.logic-grid",
            be_type_logic_grid::registrations(),
        ),
        in_process::<be_type_map::BlockTypes>("be3.map", be_type_map::registrations()),
        in_process::<be_type_example_pan_zoom::BlockTypes>(
            "be3.pan_zoom",
            be_type_example_pan_zoom::registrations(),
        ),
        in_process::<be_type_pdf::BlockTypes>("be3.pdf", be_type_pdf::registrations()),
        in_process::<be_type_presentation::BlockTypes>(
            "be3.presentation",
            be_type_presentation::registrations(),
        ),
        in_process::<be_type_scene_3d::BlockTypes>(
            "be3.scene-3d",
            be_type_scene_3d::registrations(),
        ),
        in_process::<be_type_settings::BlockTypes>(
            "be3.settings",
            be_type_settings::registrations(),
        ),
        in_process::<be_type_terminal::BlockTypes>(
            "be3.terminal",
            be_type_terminal::registrations(),
        ),
        in_process::<be_type_text::BlockTypes>("be3.text", be_type_text::registrations()),
        in_process::<be_type_example_triangle::BlockTypes>(
            "be3.triangle",
            be_type_example_triangle::registrations(),
        ),
        in_process::<be_type_video::BlockTypes>("be3.video", be_type_video::registrations()),
        in_process::<be_type_workspace::BlockTypes>(
            "be3.workspace-ui",
            be_type_workspace::registrations(),
        ),
    ]
}

pub fn in_process<G: Guest + 'static>(name: &str, registrations: Vec<Registration>) -> Source {
    be_block_type_host::in_process::<G>(name).declaring(
        registrations
            .iter()
            .map(|registration| registration.info().content_type)
            .collect(),
    )
}

pub fn knows(content_type: Uuid) -> bool {
    [
        be_type_audio::block_types(),
        be_type_example_beui_gallery::block_types(),
        be_type_browser_tab::block_types(),
        be_type_checklist::block_types(),
        be_type_example_counter::block_types(),
        be_type_database::block_types(),
        be_type_deterministic_game::block_types(),
        be_type_image::block_types(),
        be_type_infinite_canvas::block_types(),
        be_type_logic_grid::block_types(),
        be_type_map::block_types(),
        be_type_example_pan_zoom::block_types(),
        be_type_pdf::block_types(),
        be_type_presentation::block_types(),
        be_type_scene_3d::block_types(),
        be_type_settings::block_types(),
        be_type_terminal::block_types(),
        be_type_text::block_types(),
        be_type_example_triangle::block_types(),
        be_type_video::block_types(),
        be_type_workspace::block_types(),
    ]
    .iter()
    .any(|module| module.knows(content_type))
}
