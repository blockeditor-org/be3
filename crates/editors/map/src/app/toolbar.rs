use std::rc::Rc;

use block_editor_plugin::Toolbar;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{
    ICON_ADD, ICON_CROP_FREE, ICON_REFRESH, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_plugin::beui::reactive::{Prop, Show, clone, component, create_memo, view};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Caption, IconButton, use_theme};

use super::state::{MapState, ZOOM_STEP};

#[component]
pub(crate) fn MapToolbar(state: Rc<MapState>, shown: Prop<bool>) -> NodeId {
    let add = Rc::clone(&state);
    let out = Rc::clone(&state);
    let inward = Rc::clone(&state);
    let whole = state.editor().clone();
    let fit = Rc::clone(&state);
    let reload = Rc::clone(&state);
    let regionless = create_memo(clone!(state -> move || state.preview_region.get().is_none()));
    let error = state.last_error.clone();
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let theme = use_theme();

    view! {
        <Toolbar shown={shown}>
            <IconButton
                glyph={ICON_ADD.to_owned()}
                label="Add a point of interest at the centre of the view"
                @test_id={"map.add-point"}
                on_click={move || add.open_picker(None)}
            />
            <IconButton
                glyph={ICON_ZOOM_OUT.to_owned()}
                label="Zoom out"
                @test_id={"map.zoom-out"}
                on_click={move || out.zoom(1.0 / ZOOM_STEP)}
            />
            <IconButton
                glyph={ICON_ZOOM_IN.to_owned()}
                label="Zoom in"
                @test_id={"map.zoom-in"}
                on_click={move || inward.zoom(ZOOM_STEP)}
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
                on_click={move || fit.request_fit()}
            />
            <IconButton
                glyph={ICON_REFRESH.to_owned()}
                label="Reload tiles"
                @test_id={"map.reload"}
                on_click={move || reload.reload_tiles()}
            />
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
        </Toolbar>
    }
}
