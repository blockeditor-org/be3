use block_editor_beui::be_block::CanvasContent;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{BlockQuery, Loaded};
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

pub(crate) mod actions;
pub(crate) mod canvas;
pub(crate) mod components;
pub(crate) mod input;
pub(crate) mod layers;
pub(crate) mod menu;
pub(crate) mod overlay;
pub(crate) mod paint;
pub(crate) mod selection_bar;
pub(crate) mod sidebar;
pub(crate) mod state;
pub(crate) mod text;
pub(crate) mod toolbar;

use actions::canvas_actions;
use canvas::CanvasStage;
use sidebar::CanvasSidebar;
use state::CanvasState;
use toolbar::{ImportError, ToolDock, ZoomPill};

use crate::geometry::{MIN_SIZE, embedded_region};

pub struct CanvasApp;

impl block_editor_beui::BeuiApp for CanvasApp {
    fn view(editor: Editor) -> NodeId {
        let loaded =
            editor.loaded::<block_editor_beui::be_block::CanvasContent>(&[BlockQuery::References(
                editor.block_id(),
            )]);
        view! {
            <Loaded loaded={loaded}>
                <CanvasEditor editor={editor} />
            </Loaded>
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        let loaded =
            editor.loaded::<block_editor_beui::be_block::CanvasContent>(&[BlockQuery::References(
                editor.block_id(),
            )]);
        view! {
            <Loaded loaded={loaded}>
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
    let chrome = editor.chrome_shown();
    let anchor = content;
    let docked = chrome;
    let side = chrome;
    let zoom_anchor = content;
    let scope = NodeRef::new();
    action_scope(&scope);
    provide_context(canvas_actions(state));
    view! {
        <List @node_ref=&scope spacing=0.0>
            <ImportError state={state} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <CanvasStage @sizing=ItemSize::Percent(100.0) @node_ref={&content} state={state} />
                <CanvasSidebar state={state} shown={side} />
            </List>
            <ToolDock state={state} anchor={anchor} shown={docked} />
            <ZoomPill state={state} anchor={zoom_anchor} shown={chrome} />
        </List>
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
        let region = embedded_region(&state.entities.get()).size();
        editor.set_intrinsic_size(Some(Vec2::new(
            region.x.max(MIN_SIZE),
            region.y.max(MIN_SIZE),
        )));
    });
}
