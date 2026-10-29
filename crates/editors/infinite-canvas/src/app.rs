use block_editor_beui::Fit;
use infinite_canvas_types::CanvasContent;

use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::dock::styled::{DockBar, DockPane, DockTab, Docking, DockingLayout};
use block_editor_beui::beui::dock::unstyled::{DockBarPlacement, DockEdge};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Creation, Editor};
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
pub(crate) mod overlay;
pub(crate) mod paint;
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
use sidebar::CanvasSidebar;
use state::CanvasState;
use toolbar::{ImportError, STAGE, SelectionDock, TOOLS, ToolBar, ZoomPill, tools_open};
use touch::TouchSetting;

use crate::geometry::MIN_SIZE;
use beui::{Direction, ItemSize};

pub struct CanvasApp;

#[component]
impl block_editor_beui::BeuiApp for CanvasApp {
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
        chrome.get() && state.profile.get() == infinite_canvas_types::CanvasProfile::Vector
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
        <Docking layout>
            <DockPane id="canvas" bare=true>
                <DockTab id=STAGE title="Canvas">
                    <List @node_ref=&scope spacing=Space::None>
                        <TouchSetting editor choice={touch} />
                        <ImportError state={state} />
                        <ExportDialog exporter />
                        <List
                            @sizing=ItemSize::Percent(100.0)
                            direction=Direction::Horizontal
                            spacing=Space::None
                        >
                            <Layers @sizing=ItemSize::Percent(100.0)>
                                <CanvasStage @node_ref={&content} state={state} />
                                <SelectionDock state layout open bar={bar_size.read_only()} />
                            </Layers>
                            <CanvasSidebar state={state} shown={inspected} />
                        </List>
                        <ZoomPill state={state} anchor={content} shown={chrome} />
                    </List>
                </DockTab>
            </DockPane>
            <DockBar id=TOOLS parent=STAGE placement={DockBarPlacement::Over(DockEdge::Bottom)}>
                <ToolBar state open size={bar_size} />
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
