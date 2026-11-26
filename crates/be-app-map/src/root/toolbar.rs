use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::icons::{ICON_PLUS, ICON_REFRESH_CW, ICON_SCAN};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;

use super::state::MapState;

#[component]
pub(crate) fn MapToolbar(state: MapState) -> NodeId {
    let error = state.last_error;
    let add = NodeRef::new();

    view! {
        <DockBarSurface spacing=Space::Md @test_id={"map.toolbar"}>
            <Frame @test_id={"map.toolbar.grip"}>
                <DockBarGrip />
            </Frame>
            <Button
                icon_only=true
                @node_ref={&add}
                glyph=ICON_PLUS
                label="Add a point of interest at the centre of the view"
                @test_id={"map.add-point"}
                on_click={move || state.open_picker(None, add)}
            />
            <Button
                icon_only=true
                glyph=ICON_SCAN
                label="Zoom to the preview region"
                disabled={move || state.preview_region.with(Option::is_none)}
                @test_id={"map.zoom-region"}
                on_click={move || state.request_fit()}
            />
            <Button
                icon_only=true
                glyph=ICON_REFRESH_CW
                label="Reload tiles"
                @test_id={"map.reload"}
                on_click={move || state.reload_tiles()}
            />
            <Show value={error}>|reason: ReadSignal<String>|
                <Text style=TextStyle::CAPTION color=TextColor::Danger @test_id={"map.error"}>
                    {reason}
                </Text>
            </Show>
        </DockBarSurface>
    }
}
