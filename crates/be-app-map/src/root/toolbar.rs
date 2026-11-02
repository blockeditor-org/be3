use be_sdk_beui::beui::icons::{
    ICON_PLUS, ICON_REFRESH_CW, ICON_SCAN, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::Fitting;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};

use super::state::{MapState, ZOOM_STEP};

#[component]
pub(crate) fn MapToolbar(
    state: MapState,
    fitting: Fitting,
) -> NodeId {
    let regionless = create_memo(move || state.preview_region.get().is_none());
    let error = state.last_error;
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());
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
                glyph=ICON_ZOOM_OUT
                label="Zoom out"
                @test_id={"map.zoom-out"}
                on_click={move || state.zoom(1.0 / ZOOM_STEP)}
            />
            <Button
                icon_only=true
                glyph=ICON_ZOOM_IN
                label="Zoom in"
                @test_id={"map.zoom-in"}
                on_click={move || state.zoom(ZOOM_STEP)}
            />
            <Button
                label="Whole world"
                variant=ButtonVariant::Secondary
                @test_id={"map.fit"}
                on_click={move || fitting.fit()}
            />
            <Button
                icon_only=true
                glyph=ICON_SCAN
                label="Zoom to the preview region"
                disabled={regionless}
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
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
            </Show>
        </DockBarSurface>
    }
}
