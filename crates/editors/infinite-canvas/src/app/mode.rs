use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::dock::styled::{DockBarGrip, DockBarSurface, use_dock_bar};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::narrow_chrome;
use infinite_canvas_types::CanvasProfile;

use super::actions::CanvasActions;
use super::state::CanvasState;

pub(crate) const MODE: &str = "mode";

#[component]
pub(crate) fn ModeBar(open: Memo<bool>) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let narrow = narrow_chrome();
    let shown = create_memo(move || open.get() && !narrow.get());
    let bar = use_dock_bar();
    let direction =
        create_memo(move || bar.map_or(beui::Direction::Horizontal, |bar| bar.direction()));
    let content = Render::new(move |()| {
        let actions = actions.clone();
        view! {
            <DockBarSurface @test_id={"infinite-canvas.mode"}>
                <Frame @test_id={"infinite-canvas.mode.grip"}>
                    <DockBarGrip />
                </Frame>
                <List direction={direction} spacing=Space::None>
                    <ToggleButton
                        icon_only=true
                        action={actions.prototyping}
                        @test_id={"infinite-canvas.mode.prototyping"}
                    />
                    <ToggleButton
                        icon_only=true
                        action={actions.vector}
                        @test_id={"infinite-canvas.mode.vector"}
                    />
                    <ToggleButton
                        icon_only=true
                        action={actions.raster_profile}
                        @test_id={"infinite-canvas.mode.raster"}
                    />
                </List>
            </DockBarSurface>
        }
    })
    .once();
    view! {
        <List spacing=Space::None>
            <ShowKeepAlive condition={shown}>{content.call(())}</ShowKeepAlive>
        </List>
    }
}

#[component]
pub(crate) fn ModeMenu(state: CanvasState) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let profile = state.profile;
    let glyphs = [
        actions.prototyping.glyph(),
        actions.vector.glyph(),
        actions.raster_profile.glyph(),
    ];
    let glyph = create_memo(move || match profile.get() {
        CanvasProfile::Prototyping => glyphs[0],
        CanvasProfile::Vector => glyphs[1],
        CanvasProfile::Raster => glyphs[2],
    });
    let label = create_memo(move || format!("{} profile", profile.get().name()));
    let nodes = vec![
        MenuNode::labelled(&actions.prototyping, "Prototyping"),
        MenuNode::labelled(&actions.vector, "Vector"),
        MenuNode::labelled(&actions.raster_profile, "Raster"),
    ];
    view! {
        <MenuButton
            icon_only=true
            variant=ButtonVariant::Ghost
            label={label}
            glyph={glyph}
            @test_id={"infinite-canvas.mode.menu"}
            items={view! {
                <MenuNodes nodes test_prefix={"infinite-canvas.mode".to_owned()} />
            }}
        />
    }
}
