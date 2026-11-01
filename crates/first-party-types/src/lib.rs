use be_types_host::Source;
use block_guest_abi::Guest;
use block_type_guest::Registration;
use uuid::Uuid;

pub fn sources() -> Vec<Source> {
    vec![
        in_process::<audio_block_types::BlockTypes>(
            "be3.audio",
            audio_block_types::registrations(),
        ),
        in_process::<beui_gallery_types::BlockTypes>(
            "be3.beui_gallery",
            beui_gallery_types::registrations(),
        ),
        in_process::<browser_tab_types::BlockTypes>(
            "be3.browser-tab",
            browser_tab_types::registrations(),
        ),
        in_process::<checklist_types::BlockTypes>(
            "be3.checklist",
            checklist_types::registrations(),
        ),
        in_process::<counter_types::BlockTypes>("be3.counter", counter_types::registrations()),
        in_process::<database_types::BlockTypes>("be3.database", database_types::registrations()),
        in_process::<deterministic_game_types::BlockTypes>(
            "be3.deterministic-game",
            deterministic_game_types::registrations(),
        ),
        in_process::<image_block_types::BlockTypes>(
            "be3.image",
            image_block_types::registrations(),
        ),
        in_process::<infinite_canvas_types::BlockTypes>(
            "be3.infinite-canvas",
            infinite_canvas_types::registrations(),
        ),
        in_process::<logic_grid_types::BlockTypes>(
            "be3.logic-grid",
            logic_grid_types::registrations(),
        ),
        in_process::<map_types::BlockTypes>("be3.map", map_types::registrations()),
        in_process::<pan_zoom_types::BlockTypes>("be3.pan_zoom", pan_zoom_types::registrations()),
        in_process::<pdf_types::BlockTypes>("be3.pdf", pdf_types::registrations()),
        in_process::<presentation_types::BlockTypes>(
            "be3.presentation",
            presentation_types::registrations(),
        ),
        in_process::<scene_3d_types::BlockTypes>("be3.scene-3d", scene_3d_types::registrations()),
        in_process::<settings_types::BlockTypes>("be3.settings", settings_types::registrations()),
        in_process::<terminal_types::BlockTypes>("be3.terminal", terminal_types::registrations()),
        in_process::<text_block_types::BlockTypes>("be3.text", text_block_types::registrations()),
        in_process::<triangle_types::BlockTypes>("be3.triangle", triangle_types::registrations()),
        in_process::<video_block_types::BlockTypes>(
            "be3.video",
            video_block_types::registrations(),
        ),
        in_process::<workspace_ui_types::BlockTypes>(
            "be3.workspace-ui",
            workspace_ui_types::registrations(),
        ),
    ]
}

pub fn in_process<G: Guest + 'static>(name: &str, registrations: Vec<Registration>) -> Source {
    be_types_host::in_process::<G>(name).declaring(
        registrations
            .iter()
            .map(|registration| registration.info().content_type)
            .collect(),
    )
}

pub fn knows(content_type: Uuid) -> bool {
    [
        audio_block_types::block_types(),
        beui_gallery_types::block_types(),
        browser_tab_types::block_types(),
        checklist_types::block_types(),
        counter_types::block_types(),
        database_types::block_types(),
        deterministic_game_types::block_types(),
        image_block_types::block_types(),
        infinite_canvas_types::block_types(),
        logic_grid_types::block_types(),
        map_types::block_types(),
        pan_zoom_types::block_types(),
        pdf_types::block_types(),
        presentation_types::block_types(),
        scene_3d_types::block_types(),
        settings_types::block_types(),
        terminal_types::block_types(),
        text_block_types::block_types(),
        triangle_types::block_types(),
        video_block_types::block_types(),
        workspace_ui_types::block_types(),
    ]
    .iter()
    .any(|module| module.knows(content_type))
}
