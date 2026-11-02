use be_sdk_beui::Fit;
use be_type_map::MapContent;

use be_sdk_beui::Loaded;
use be_sdk_beui::beui::dock::styled::{
    DockBar, DockPane, DockSplit, DockTab, Docking, DockingLayout,
};
use be_sdk_beui::beui::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{Creation, Editor, Sidebar, narrow_chrome};
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod labels;
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod tiles;
pub(crate) mod toolbar;

use beui::ItemSize;
use canvas::MapCanvas;
use sidebar::{MapInspector, MapSidebar};
use state::{MapState, WORLD_POINTS};
use toolbar::MapToolbar;

const STAGE: &str = "stage";
const INSPECTOR: &str = "inspector";
const TOOLBAR: &str = "toolbar";
const INSPECTOR_SHARE: f32 = 0.3;

pub struct MapRoot;

#[component]
impl be_sdk_beui::BeuiRoot for MapRoot {
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
    be_sdk_beui::beui::reactive::create_effect(move || {
        let aspect = crate::geo::region_aspect_ratio(region.get()).max(0.01);
        let _ = state.block_id();
        editor.set_intrinsic_size(Some(match aspect >= 1.0 {
            true => Vec2::new(WORLD_POINTS, WORLD_POINTS / aspect),
            false => Vec2::new(WORLD_POINTS * aspect, WORLD_POINTS),
        }));
    });

    let fitting = editor.keep_fitted(|| Some(Fit::World));

    let chrome = editor.chrome_shown();
    let narrow = narrow_chrome();
    let sheet = create_memo(move || chrome.get() && narrow.get());
    let docked = create_memo(move || chrome.get() && !narrow.get());
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="map" fraction={1.0 - INSPECTOR_SHARE}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="Map">
                        <List spacing=Space::None>
                            <MapCanvas @sizing=ItemSize::Percent(100.0) state={state} />
                            <Sidebar shown={sheet}>
                                <MapSidebar state={state} />
                            </Sidebar>
                        </List>
                    </DockTab>
                </DockPane>
                <Show condition={docked}>
                    <DockPane id="inspector" locked=true>
                        <DockTab id=INSPECTOR title="Inspector">
                            <MapInspector state={state} />
                        </DockTab>
                    </DockPane>
                </Show>
            </DockSplit>
            <Show condition={chrome}>
                <DockBar
                    id=TOOLBAR
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <MapToolbar state={state} fitting={fitting} />
                </DockBar>
            </Show>
        </Docking>
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
