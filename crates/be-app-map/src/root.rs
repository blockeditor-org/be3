use be_sdk_beui::Fit;
use be_type_map::MapContent;

use be_sdk_beui::Loaded;
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{
    DockBar, DockPane, DockSplit, DockTab, Docking,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{Creation, Editor, ZoomBar};
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod labels;
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod tiles;
pub(crate) mod toolbar;

use canvas::MapCanvas;
use sidebar::MapInspector;
use state::{MapState, WORLD_POINTS};
use toolbar::MapToolbar;

const STAGE: &str = "stage";
const TOOLBAR: &str = "toolbar";
const ZOOM: &str = "zoom";

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

    editor.keep_fitted(|| Some(Fit::World));

    let chrome = editor.chrome_shown();
    view! {
        <Docking>
            <DockSplit>
                <DockPane stage=true>
                    <DockTab id=STAGE>
                        <MapCanvas state={state} />
                    </DockTab>
                </DockPane>
                <DockPane chrome=true>
                    <DockTab title="Inspector">
                        <MapInspector state={state} />
                    </DockTab>
                </DockPane>
            </DockSplit>
            <Show value={chrome}>
                <DockBar
                    id=TOOLBAR
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <MapToolbar state={state} />
                </DockBar>
                <DockBar
                    id=ZOOM
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::End)}
                >
                    <ZoomBar editor />
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
