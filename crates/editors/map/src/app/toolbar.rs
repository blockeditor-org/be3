use block_editor_beui::beui::icons::{
    ICON_ADD, ICON_CROP_FREE, ICON_REFRESH, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Toolbar, node_anchor};

use super::state::{MapState, ZOOM_STEP};

#[component]
pub(crate) fn MapToolbar(state: MapState, shown: Prop<bool>) -> DynamicSegment<ListChild> {
    let whole = state.editor();
    let regionless = create_memo(move || state.preview_region.get().is_none());
    let error = state.last_error;
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());
    let add = NodeRef::new();

    view! {
        <Toolbar shown={shown}>
            <IconButton
                @node_ref={&add}
                glyph={ICON_ADD.to_owned()}
                label="Add a point of interest at the centre of the view"
                @test_id={"map.add-point"}
                on_click={move || state.open_picker(None, node_anchor(&add))}
            />
            <IconButton
                glyph={ICON_ZOOM_OUT.to_owned()}
                label="Zoom state"
                @test_id={"map.zoom-state"}
                on_click={move || state.zoom(1.0 / ZOOM_STEP)}
            />
            <IconButton
                glyph={ICON_ZOOM_IN.to_owned()}
                label="Zoom in"
                @test_id={"map.zoom-in"}
                on_click={move || state.zoom(ZOOM_STEP)}
            />
            <Button
                label="Whole world"
                variant=ButtonVariant::Secondary
                @test_id={"map.fit"}
                on_click={move || whole.fit()}
            />
            <IconButton
                glyph={ICON_CROP_FREE.to_owned()}
                label="Zoom to the preview region"
                disabled={regionless}
                @test_id={"map.zoom-region"}
                on_click={move || state.request_fit()}
            />
            <IconButton
                glyph={ICON_REFRESH.to_owned()}
                label="Reload tiles"
                @test_id={"map.reload"}
                on_click={move || state.reload_tiles()}
            />
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
            </Show>
        </Toolbar>
    }
}
