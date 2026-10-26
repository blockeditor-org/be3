use block_editor_beui::Fit;
use map_types::MapContent;

use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{Creation, Editor, Sidebar};
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod labels;
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod tiles;
pub(crate) mod toolbar;

use canvas::MapCanvas;
use sidebar::MapSidebar;
use state::{MapState, WORLD_POINTS};
use toolbar::MapToolbar;

pub struct MapApp;

#[component]
impl block_editor_beui::BeuiApp for MapApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <MapEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <MapPreview editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&MapContent::default()))
    }
}

#[component]
fn MapEditor(editor: Editor) -> NodeId {
    let state = MapState::new(editor, false);
    state.watch();

    let region = state.displayed_region;
    block_editor_beui::beui::reactive::create_effect(move || {
        let aspect = crate::geo::region_aspect_ratio(region.get()).max(0.01);
        let _ = state.block_id();
        editor.set_intrinsic_size(Some(match aspect >= 1.0 {
            true => Vec2::new(WORLD_POINTS, WORLD_POINTS / aspect),
            false => Vec2::new(WORLD_POINTS * aspect, WORLD_POINTS),
        }));
    });

    let fitting = editor.keep_fitted(|| Some(Fit::World));

    let chrome = editor.chrome_shown();
    view! {
        <List spacing=Space::None>
            <MapToolbar state={state} fitting={fitting} shown={chrome} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=Space::None>
                <MapCanvas @sizing=ItemSize::Percent(100.0) state={state} />
                <Sidebar shown={chrome}>
                    <MapSidebar state={state} />
                </Sidebar>
            </List>
        </List>
    }
}

#[component]
fn MapPreview(editor: Editor) -> NodeId {
    let state = MapState::new(editor, true);
    state.watch();
    view! {
        <MapCanvas state={state} />
    }
}
