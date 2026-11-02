use be_sdk_beui::Fit;
use be_type_infinite_canvas::CanvasContent;

use be_sdk_beui::Loaded;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::dock::styled::{
    DockBar, DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize,
};
use be_sdk_beui::beui::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{Creation, Editor};
use uuid::Uuid;

pub(crate) mod actions;
pub(crate) mod canvas;
pub(crate) mod components;
pub(crate) mod entities;
pub(crate) mod export;
pub(crate) mod grid;
pub(crate) mod input;
pub(crate) mod layers;
pub(crate) mod menu;
pub(crate) mod mode;
pub(crate) mod overlay;
pub(crate) mod paint;
pub(crate) mod raster;
pub(crate) mod selection_bar;
pub(crate) mod sidebar;
pub(crate) mod sketch;
pub(crate) mod state;
pub(crate) mod style;
pub(crate) mod text;
pub(crate) mod toolbar;
pub(crate) mod touch;

use actions::canvas_actions;
use canvas::CanvasStage;
use export::ExportDialog;
use layers::CanvasLayers;
use mode::{MODE, ModeBar};
use raster::RasterDialogs;
use sidebar::CanvasInspector;
use state::CanvasState;
use toolbar::{ImportError, STAGE, SelectionDock, TOOLS, ToolBar, ZOOM, ZoomBar, tools_open};
use touch::TouchSetting;

use crate::geometry::MIN_SIZE;
use beui::{Direction, ItemSize};

pub(crate) const INSPECTOR: &str = "inspector";
const LAYERS: &str = "layers";
const INSPECTOR_WIDTH: f32 = 280.0;
const LAYERS_HEIGHT: f32 = 240.0;

pub struct CanvasRoot;

#[component]
impl be_sdk_beui::BeuiRoot for CanvasRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let presented = editor.presenting();
        let editing = create_memo(move || !presented.get());
        view! {
            <Root>
                <Loaded>
                    <Layers>
                        <Show condition={editing}>
                            <CanvasEditor editor={editor} />
                        </Show>
                        <Show condition={presented}>
                            <CanvasPresented editor={editor} />
                        </Show>
                    </Layers>
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <CanvasPreview editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CanvasContent::default()))
    }
}

#[component]
fn CanvasEditor(editor: Editor) -> NodeId {
    let state = CanvasState::new(editor, false);
    state.watch();

    editor.on_replace_child(move |old, new| {
        state.is_alive() && state.replace_referenced_block(old, new)
    });

    report_intrinsic_size(editor, state);

    let content = NodeRef::new();
    editor.content(&content);
    editor.keep_fitted(|| Some(Fit::Centre));
    let chrome = editor.chrome_shown();
    let inspected = create_memo(move || {
        chrome.get() && state.profile.get() != be_type_infinite_canvas::CanvasProfile::Prototyping
    });
    let scope = NodeRef::new();
    action_scope(&scope);
    let actions = canvas_actions(state);
    add_to_menu(menu::frame_menu(&actions));
    let touch = actions.touch;
    let exporter = actions.exporter;
    provide_context(actions);
    let open = tools_open(state, chrome);
    let bar_size = create_signal(Vec2::ZERO);
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="canvas" size={SplitSize::Second(INSPECTOR_WIDTH)}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="Canvas">
                        <List @node_ref=&scope spacing=Space::None>
                            <TouchSetting editor choice={touch} />
                            <ImportError state={state} />
                            <ExportDialog exporter />
                            <RasterDialogs state />
                            <Layers @sizing=ItemSize::Percent(100.0)>
                                <CanvasStage @node_ref={&content} state={state} />
                                <SelectionDock state layout open bar={bar_size.read_only()} />
                            </Layers>
                        </List>
                    </DockTab>
                </DockPane>
                <Show condition={inspected}>
                    <DockSplit
                        id="side"
                        direction=Direction::Vertical
                        size={SplitSize::Second(LAYERS_HEIGHT)}
                    >
                        <DockPane id="inspector" locked=true>
                            <DockTab id=INSPECTOR title="Inspector">
                                <CanvasInspector state={state} />
                            </DockTab>
                        </DockPane>
                        <DockPane id="layers" locked=true>
                            <DockTab id=LAYERS title="Layers">
                                <CanvasLayers state={state} />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Show>
            </DockSplit>
            <DockBar
                id=MODE
                parent=STAGE
                placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::Start)}
            >
                <ModeBar open />
            </DockBar>
            <DockBar
                id=TOOLS
                parent=STAGE
                placement={DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center)}
            >
                <ToolBar state open size={bar_size} />
            </DockBar>
            <DockBar
                id=ZOOM
                parent=STAGE
                placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::End)}
            >
                <ZoomBar state open />
            </DockBar>
        </Docking>
    }
}

#[component]
fn CanvasPresented(editor: Editor) -> NodeId {
    let state = CanvasState::new(editor, true);
    state.watch();
    let content = NodeRef::new();
    editor.content(&content);
    editor.keep_fitted(|| Some(Fit::Centre));
    provide_context(canvas_actions(state));
    view! {
        <CanvasStage @node_ref=&content state={state} />
    }
}

#[component]
fn CanvasPreview(editor: Editor) -> NodeId {
    let state = CanvasState::new(editor, true);
    state.watch();
    report_intrinsic_size(editor, state);
    provide_context(canvas_actions(state));
    view! {
        <CanvasStage state={state} />
    }
}

fn report_intrinsic_size(editor: Editor, state: CanvasState) {
    create_effect(move || {
        let region = state.entities.region().size();
        editor.set_intrinsic_size(Some(Vec2::new(
            region.x.max(MIN_SIZE),
            region.y.max(MIN_SIZE),
        )));
    });
}
