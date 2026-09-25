use block_editor_plugin::be_block::MapContent;
use std::rc::Rc;

use block_editor_plugin::beui::reactive::{Direction, ItemSize, List, component, view};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor, Sidebar};
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod tiles;
pub(crate) mod toolbar;

use canvas::MapCanvas;
use sidebar::MapSidebar;
use state::{MapState, WORLD_POINTS};
use toolbar::MapToolbar;

pub struct MapApp;

impl block_editor_plugin::BeuiApp for MapApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <MapEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <MapPreview editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&MapContent::default()))
    }
}

#[component]
fn MapEditor(editor: Editor) -> NodeId {
    let state = MapState::new(&editor, false);
    state.watch();

    let sized = Rc::clone(&state);
    let sizing = editor.clone();
    let region = state.displayed_region.clone();
    block_editor_plugin::beui::reactive::create_effect(move || {
        let aspect = crate::geo::region_aspect_ratio(region.get()).max(0.01);
        let _ = sized.block_id();
        sizing.set_intrinsic_size(Some(match aspect >= 1.0 {
            true => Vec2::new(WORLD_POINTS, WORLD_POINTS / aspect),
            false => Vec2::new(WORLD_POINTS * aspect, WORLD_POINTS),
        }));
    });

    let chrome = editor.chrome_shown();
    let bar = Rc::clone(&state);
    let canvas = Rc::clone(&state);
    view! {
        <List spacing=0.0>
            <MapToolbar state={bar} shown={chrome.clone()} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <MapCanvas @sizing=ItemSize::Percent(100.0) state={canvas} />
                <Sidebar shown={chrome}>
                    <MapSidebar state={state} />
                </Sidebar>
            </List>
        </List>
    }
}

#[component]
fn MapPreview(editor: Editor) -> NodeId {
    let state = MapState::new(&editor, true);
    state.watch();
    view! {
        <MapCanvas state={state} />
    }
}
